/*
 * Copyright (C) 2021 The Aero Project Developers.
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
use lru::LruCache;
use spin::Once;

use crate::fs::inode::{DirEntry, INodeInterface};
use crate::utils::sync::Mutex;

pub(super) static INODE_CACHE: Once<Arc<INodeCache>> = Once::new();
pub(super) static DIR_CACHE: Once<Arc<DirCache>> = Once::new();

pub trait CacheKey: Hash + Ord + Borrow<Self> + Debug {}

impl<T> CacheKey for T where T: Hash + Ord + Borrow<Self> + Debug {}

pub trait Cacheable<K: CacheKey>: Sized {
    fn cache_key(&self) -> K;
}

/// Structure representing a cache item in the cache index. See the documentation of [CacheIndex]
/// and the fields of this struct for more information.
pub struct CacheItem<K: CacheKey, V: Cacheable<K>> {
    #[allow(unused)]
    cache: Weak<Cache<K, V>>,
    value: V,
}

impl<K: CacheKey, V: Cacheable<K>> CacheItem<K, V> {
    pub fn new(cache: &Weak<Cache<K, V>>, value: V) -> Arc<Self> {
        Arc::new(Self {
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

/// Inner implementation structure for caching. This structure basically contains the
/// LRU cache of the unused entries and a hashmap of the used entries.
struct CacheIndex<K: CacheKey, V: Cacheable<K>> {
    unused: LruCache<K, Arc<CacheItem<K, V>>>,
    used: hashbrown::HashMap<K, Weak<CacheItem<K, V>>>,
}

/// Structure representing a cache with a key of `K` and value of `V`. The cache
/// key is used to get the cache from the cache index. This structure basically contains
/// the cache index (protected by a mutex) and a weak self reference to itself.
pub struct Cache<K: CacheKey, V: Cacheable<K>> {
    index: Mutex<CacheIndex<K, V>>,

    #[allow(unused)]
    self_ref: Weak<Cache<K, V>>,
}

impl<K: CacheKey, V: Cacheable<K>> Cache<K, V> {
    /// Creates a new cache with the provided that holds at most `capacity` items.
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new_cyclic(|this| Cache::<K, V> {
            index: Mutex::new(CacheIndex {
                unused: LruCache::new(capacity),
                used: hashbrown::HashMap::new(),
            }),
            self_ref: this.clone(),
        })
    }

    /// This function is responsible for clearning the used and unused.
    pub fn clear(&self) {
        let mut index_mut = self.index.lock();

        index_mut.unused.clear();
        index_mut.used.clear();
    }

    pub fn make_item_cached(&self, value: V) -> Arc<CacheItem<K, V>> {
        let item = CacheItem::<K, V>::new(&self.self_ref, value);

        self.index
            .lock()
            .used
            .insert(item.cache_key(), Arc::downgrade(&item));

        item
    }

    pub fn make_item_no_cache(&self, value: V) -> Arc<CacheItem<K, V>> {
        CacheItem::<K, V>::new(&Weak::default(), value)
    }

    pub fn get(&self, key: K) -> Option<Arc<CacheItem<K, V>>> {
        let mut index = self.index.lock();

        if let Some(entry) = index.used.get(&key) {
            return entry.clone().upgrade();
        } else if let Some(entry) = index.unused.pop(&key) {
            return Some(entry.clone());
        } else {
            None
        }
    }
}

pub type INodeCacheKey = (usize, usize);
pub type INodeCache = Cache<INodeCacheKey, CachedINode>;
pub type INodeCacheItem = Arc<CacheItem<INodeCacheKey, CachedINode>>;
pub type INodeCacheWeakItem = Weak<CacheItem<INodeCacheKey, CachedINode>>;

/// The cache key for the directory entry cache used to get the cache item. The cache key
/// is the tuple of the parent's cache marker (akin [usize]) and the name of the directory entry
/// (akin [String]).
pub type DirCacheKey = (usize, String);
pub type DirCache = Cache<DirCacheKey, DirEntry>;
pub type DirCacheItem = Arc<CacheItem<DirCacheKey, DirEntry>>;

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
        todo!()
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

// NOTE: Needs to be implemented inside `DirCacheItem` since the following functions
// require a reference-counting pointer to the directory item. Annd since we are using
// Arc which is from core we will need to extract these functions into a trait instead. Oh
// well...
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

pub(super) fn icache() -> &'static Arc<INodeCache> {
    INODE_CACHE
        .get()
        .expect("`icache` was invoked before it was initialized")
}

pub(super) fn dcache() -> &'static Arc<DirCache> {
    DIR_CACHE
        .get()
        .expect("`dcache` was invoked before it was initialized")
}

/// This function is responsible for initializing the inode cache.
pub fn init() {
    INODE_CACHE.call_once(|| INodeCache::new(256));
    DIR_CACHE.call_once(|| DirCache::new(256));
}
