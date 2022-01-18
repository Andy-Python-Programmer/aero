/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

//! General implementation for file system caching. Stuff like inode needs to be cached
//! to improve performance and in this case looking up inode data from an IO device such
//! as a disk is very slow, so storing previously accessed inode data in memory makes file
//! system access much quicker.
//!
//! ## Notes
//! * <https://wiki.osdev.org/File_Systems>

use core::borrow::Borrow;
use core::fmt::Debug;
use core::hash::Hash;
use core::ops;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::sync::Weak;

use alloc::vec::Vec;
use spin::Once;

use crate::fs::inode::{DirEntry, INodeInterface};
use crate::utils::sync::Mutex;

pub(super) static INODE_CACHE: Once<Arc<INodeCache>> = Once::new();
pub(super) static DIR_CACHE: Once<Arc<DirCache>> = Once::new();

// NOTE: We require a custom wrapper around [`Arc`] and [`Weak`] since we need to be able
// to move the cache item from the used list to the unused list when the cache item is dropped.
// This would require us to implement a custom drop handler implementation.
pub struct CacheArc<T: CacheDropper>(Arc<T>);

impl<T: CacheDropper> CacheArc<T> {
    /// Constructs a new `CacheArc<T>`.
    #[inline]
    pub fn new(data: T) -> Self {
        Self(Arc::new(data))
    }

    pub fn downgrade(&self) -> CacheWeak<T> {
        CacheWeak(Arc::downgrade(&self.0))
    }
}

impl<T: CacheDropper> core::ops::Deref for CacheArc<T> {
    type Target = Arc<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: CacheDropper> Clone for CacheArc<T> {
    /// Makes a clone of the `CacheArc<T>` pointer.
    ///
    /// This creates another pointer to the same allocation, increasing the
    /// strong reference count.
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: CacheDropper> Drop for CacheArc<T> {
    /// Drops the `ArcCache`.
    ///
    /// This will decrement the strong reference count. If the strong reference
    /// count reaches zero then the only other references (if any) are
    /// [`CacheWeak`], so we `drop` the inner value.
    fn drop(&mut self) {
        let strong_count = Arc::strong_count(&self.0);

        if strong_count == 1 {
            self.drop_this(self.0.clone());
        }
    }
}

impl<T: CacheDropper> From<Arc<T>> for CacheArc<T> {
    /// Converts an `Arc<T>` into a `CacheArc<T>`.
    fn from(data: Arc<T>) -> Self {
        Self(data)
    }
}

pub struct CacheWeak<T: CacheDropper>(Weak<T>);

impl<T: CacheDropper> CacheWeak<T> {
    /// Constructs a new `Weak<T>`, without allocating any memory.
    /// Calling [`upgrade`] on the return value always gives [`None`].
    pub fn new() -> Self {
        Self(Weak::new())
    }

    /// Attempts to upgrade the Weak pointer to an Arc, delaying dropping of the inner
    /// value if successful.
    ///
    /// Returns [`None`] if the inner value has since been dropped.
    pub fn upgrade(&self) -> Option<CacheArc<T>> {
        Some(self.0.upgrade()?.into())
    }
}

impl<T: CacheDropper> Clone for CacheWeak<T> {
    /// Makes a clone of the `CacheWeak<T>` pointer.
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: CacheDropper> Default for CacheWeak<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

pub trait CacheDropper {
    fn drop_this(&self, this: Arc<Self>);
}

pub trait CacheKey: Hash + Ord + Borrow<Self> + Debug {}

impl<T> CacheKey for T where T: Hash + Ord + Borrow<Self> + Debug {}

pub trait Cacheable<K: CacheKey>: Sized {
    fn cache_key(&self) -> K;
}

/// Structure representing a cache item in the cache index. See the documentation of [CacheIndex]
/// and the fields of this struct for more information.
pub struct CacheItem<K: CacheKey, V: Cacheable<K>> {
    cache: Weak<Cache<K, V>>,
    value: V,
}

impl<K: CacheKey, V: Cacheable<K>> CacheItem<K, V> {
    /// Constructs a new `CacheItem<K, V>`.
    #[inline]
    pub fn new(cache: &Weak<Cache<K, V>>, value: V) -> CacheArc<Self> {
        CacheArc::new(Self {
            cache: cache.clone(),
            value,
        })
    }
}

impl<K: CacheKey, V: Cacheable<K>> ops::Deref for CacheItem<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

unsafe impl<K: CacheKey, V: Cacheable<K>> Sync for CacheItem<K, V> {}

struct CacheIndex<K: CacheKey, V: Cacheable<K>> {
    used: hashbrown::HashMap<K, Weak<CacheItem<K, V>>>,
}

/// Structure representing a cache with a key of `K` and value of `V`. The cache
/// key is used to get the cache from the cache index. This structure basically contains
/// the cache index (protected by a mutex) and a weak self reference to itself.
pub struct Cache<K: CacheKey, V: Cacheable<K>> {
    index: Mutex<CacheIndex<K, V>>,
    self_ref: Weak<Cache<K, V>>,
}

impl<K: CacheKey, V: Cacheable<K>> Cache<K, V> {
    pub(super) fn new() -> Arc<Self> {
        Arc::new_cyclic(|this| Cache::<K, V> {
            index: Mutex::new(CacheIndex {
                used: hashbrown::HashMap::new(),
            }),
            self_ref: this.clone(),
        })
    }

    pub(super) fn clear(&self) {
        let mut index_mut = self.index.lock();

        index_mut.used.clear();
    }

    pub(super) fn make_item_cached(&self, value: V) -> CacheArc<CacheItem<K, V>> {
        let item = CacheItem::<K, V>::new(&self.self_ref, value);

        self.index
            .lock()
            .used
            .insert(item.cache_key(), Arc::downgrade(&item));

        item
    }

    pub(super) fn make_item_no_cache(&self, value: V) -> CacheArc<CacheItem<K, V>> {
        CacheItem::<K, V>::new(&Weak::default(), value)
    }

    pub(super) fn get(&self, key: K) -> Option<CacheArc<CacheItem<K, V>>> {
        let index = self.index.lock();

        if let Some(entry) = index.used.get(&key) {
            Some(CacheArc::from(entry.upgrade()?))
        } else {
            None
        }
    }

    pub fn log(&self) {
        log::debug!("Cache:");

        log::debug!("\t Used entries:    {}", self.index.lock().used.len());
        for item in self.index.lock().used.iter() {
            log::debug!("\t\t {:?} -> {:?}", item.0, item.1.strong_count());
        }
    }

    /// Removes the item with the provided `key` from the cache.
    pub(super) fn remove(&self, key: &K) {
        let mut index = self.index.lock();

        index.used.remove(key);
    }

    fn mark_item_unused(&self, item: CacheArc<CacheItem<K, V>>) {
        let mut this = self.index.lock();
        let key = item.cache_key();

        this.used.remove(&key);
    }
}

impl<K: CacheKey, T: Cacheable<K>> CacheDropper for CacheItem<K, T> {
    fn drop_this(&self, this: Arc<Self>) {
        if let Some(cache) = self.cache.upgrade() {
            cache.mark_item_unused(this.into());
        }
    }
}

pub type INodeCacheKey = (usize, usize);
pub type INodeCache = Cache<INodeCacheKey, CachedINode>;
pub type INodeCacheItem = CacheArc<CacheItem<INodeCacheKey, CachedINode>>;
pub type INodeCacheWeakItem = CacheWeak<CacheItem<INodeCacheKey, CachedINode>>;

/// The cache key for the directory entry cache used to get the cache item. The cache key
/// is the tuple of the parent's cache marker (akin [usize]) and the name of the directory entry
/// (akin [String]).
pub type DirCacheKey = (usize, String);
pub type DirCache = Cache<DirCacheKey, DirEntry>;
pub type DirCacheItem = CacheArc<CacheItem<DirCacheKey, DirEntry>>;

pub struct CachedINode(Arc<dyn INodeInterface>);

impl CachedINode {
    pub fn new(inode: Arc<dyn INodeInterface>) -> Self {
        Self(inode)
    }

    pub fn inner(&self) -> &Arc<dyn INodeInterface> {
        &self.0
    }
}

impl ops::Deref for CachedINode {
    type Target = Arc<dyn INodeInterface>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Cacheable<INodeCacheKey> for CachedINode {
    fn cache_key(&self) -> INodeCacheKey {
        (
            Weak::as_ptr(&self.weak_filesystem().unwrap()) as *const () as usize,
            self.metadata().unwrap().id,
        )
    }
}

impl Cacheable<DirCacheKey> for DirEntry {
    fn cache_key(&self) -> DirCacheKey {
        let this = self.data.lock();

        if let Some(parent) = this.parent.as_ref() {
            (parent.cache_marker, this.name.clone())
        } else {
            (0x00, this.name.clone())
        }
    }
}

pub trait DirCacheImpl {
    fn absolute_path_str(&self) -> String;
}

impl DirCacheImpl for DirCacheItem {
    fn absolute_path_str(&self) -> String {
        let mut current_entry = Some(self.clone());
        let mut path_nodes = Vec::new();
        let mut result = String::new();

        // We need to collect all of the path nodes, reverse them and then join them
        // with the path separator.
        while let Some(entry) = current_entry {
            path_nodes.push(entry.name());
            current_entry = entry.data.lock().parent.clone();
        }

        for node in path_nodes.iter().rev() {
            result.push_str(node);

            // If we are not at the root node, we need to add the path separator.
            if node != "/" {
                result.push('/');
            }
        }

        result
    }
}

#[inline]
pub fn clear_inode_cache() {
    INODE_CACHE.get().map(|cache| cache.clear());
}

#[inline]
pub fn clear_dir_cache() {
    DIR_CACHE.get().map(|cache| cache.clear());
}

pub fn icache() -> &'static Arc<INodeCache> {
    INODE_CACHE
        .get()
        .expect("`icache` was invoked before it was initialized")
}

pub fn dcache() -> &'static Arc<DirCache> {
    DIR_CACHE
        .get()
        .expect("`dcache` was invoked before it was initialized")
}

/// This function is responsible for initializing the inode cache.
pub fn init() {
    INODE_CACHE.call_once(|| INodeCache::new());
    DIR_CACHE.call_once(|| DirCache::new());
}
