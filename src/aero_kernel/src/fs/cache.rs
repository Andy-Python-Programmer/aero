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
use core::num::NonZeroUsize;
use core::ops;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

use alloc::sync::Arc;
use alloc::sync::Weak;

use alloc::vec::Vec;
use spin::Once;

use crate::fs::inode::{DirEntry, INodeInterface};
use crate::utils::sync::Mutex;

use super::FileSystem;

pub static INODE_CACHE: Once<Arc<INodeCache>> = Once::new();
pub static DIR_CACHE: Once<Arc<DirCache>> = Once::new();

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

pub struct CacheItem<K: CacheKey, V: Cacheable<K>> {
    cache: Weak<Cache<K, V>>,
    value: V,
    /// Whether the cache item has active strong references associated
    /// with it.
    used: AtomicBool,
}

impl<K: CacheKey, V: Cacheable<K>> CacheItem<K, V> {
    pub fn new(cache: &Weak<Cache<K, V>>, value: V) -> CacheArc<Self> {
        CacheArc::new(Self {
            cache: cache.clone(),
            value,
            used: AtomicBool::new(false),
        })
    }

    pub fn is_used(&self) -> bool {
        self.used.load(Ordering::SeqCst)
    }

    pub fn set_used(&self, yes: bool) {
        self.used.store(yes, Ordering::SeqCst);
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
    /// Cache items that are longer have any active strong references associated
    /// with them. These are stored in the cache index so, if the item is
    /// accessed again, we can re-use it; reducing required memory allocation
    /// and I/O (if applicable).
    unused: lru::LruCache<K, Arc<CacheItem<K, V>>>,
}

pub struct Cache<K: CacheKey, V: Cacheable<K>> {
    index: Mutex<CacheIndex<K, V>>,
    self_ref: Weak<Cache<K, V>>,
}

impl<K: CacheKey, V: Cacheable<K>> Cache<K, V> {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|this| Cache::<K, V> {
            index: Mutex::new(CacheIndex {
                used: hashbrown::HashMap::new(),
                unused: lru::LruCache::new(NonZeroUsize::new(512).unwrap()),
            }),
            self_ref: this.clone(),
        })
    }

    pub fn clear(&self) {
        let mut index_mut = self.index.lock();

        index_mut.unused.clear();
        index_mut.used.clear();
    }

    pub fn make_item_cached(&self, value: V) -> CacheArc<CacheItem<K, V>> {
        let item = CacheItem::<K, V>::new(&self.self_ref, value);

        self.index
            .lock()
            .used
            .insert(item.cache_key(), Arc::downgrade(&item));

        item.set_used(true);
        item
    }

    pub fn make_item_no_cache(&self, value: V) -> CacheArc<CacheItem<K, V>> {
        CacheItem::<K, V>::new(&Weak::default(), value)
    }

    pub fn get(&self, key: K) -> Option<CacheArc<CacheItem<K, V>>> {
        let mut index = self.index.lock();

        if let Some(entry) = index.used.get(&key) {
            let entry = entry.upgrade()?;
            Some(CacheArc::from(entry))
        } else if let Some(entry) = index.unused.pop(&key) {
            entry.set_used(true);
            index.used.insert(key, Arc::downgrade(&entry));

            Some(entry.into())
        } else {
            None
        }
    }

    pub fn log(&self) {
        let index = self.index.lock();

        log::debug!("Cache:");

        log::debug!("\t Used entries:    {}", index.used.len());
        for (key, item) in index.used.iter() {
            log::debug!("\t\t {:?} -> {:?}", key, item.strong_count());
        }

        log::debug!("\t Unused entries:  {}", index.unused.len());
        for (key, item) in index.unused.iter() {
            log::debug!("\t\t {:?} -> {:?}", key, Arc::strong_count(item))
        }
    }

    /// Removes the item with the provided `key` from the cache.
    pub fn remove(&self, key: &K) {
        let mut index = self.index.lock();

        if index.used.remove(key).is_none() {
            let _ = index.unused.pop(key);
        }
    }

    fn mark_item_unused(&self, item: CacheArc<CacheItem<K, V>>) {
        item.set_used(false);

        let mut index = self.index.lock_irq();
        let key = item.cache_key();

        assert!(index.used.remove(&key).is_some());
        index.unused.put(key, item.0.clone());
    }
}

impl<K: CacheKey, T: Cacheable<K>> CacheDropper for CacheItem<K, T> {
    fn drop_this(&self, this: Arc<Self>) {
        if let Some(cache) = self.cache.upgrade() {
            if self.is_used() {
                cache.mark_item_unused(this.into());
            }
        }
    }
}

pub type INodeCacheKey = (usize, usize);
pub type INodeCache = Cache<INodeCacheKey, CachedINode>;
pub type INodeCacheItem = CacheArc<CacheItem<INodeCacheKey, CachedINode>>;
pub type INodeCacheWeakItem = CacheWeak<CacheItem<INodeCacheKey, CachedINode>>;

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
        INodeCacheItem::make_key(self.weak_filesystem().unwrap(), self.metadata().unwrap().id)
    }
}

impl INodeCacheItem {
    pub fn make_key(fs: Weak<dyn FileSystem>, id: usize) -> INodeCacheKey {
        (Weak::as_ptr(&fs) as *const () as usize, id)
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
