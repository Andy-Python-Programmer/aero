/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
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

use alloc::sync::Arc;
use alloc::sync::Weak;
use spin::Mutex;

use lru::LruCache;
use spin::Once;

use super::inode::INodeInterface;

pub(super) static INODE_CACHE: Once<Arc<INodeCache>> = Once::new();

pub trait CacheKey: Hash + Ord + Borrow<Self> + Debug {}

impl<T> CacheKey for T where T: Hash + Ord + Borrow<Self> + Debug {}

pub trait Cacheable<K: CacheKey>: Sized {}

/// Structure representing a cache item in the cache index. See the documentation of [CacheIndex]
/// and the fields of this struct for more information.
pub struct CacheItem<K: CacheKey, T: Cacheable<K>> {
    cache: Weak<Cache<K, T>>,
}

/// Inner implementation structure for caching. This structure basically contains the
/// LRU cache of the unused entries and a hashmap of the used entries.
struct CacheIndex<K: CacheKey, V: Cacheable<K>> {
    unused: LruCache<K, Arc<CacheItem<K, V>>>,
    used: hashbrown::HashMap<K, Arc<CacheItem<K, V>>>,
}

/// Structure representing a cache with a key of `K` and value of `V`. The cache
/// key is used to get the cache from the cache index. This structure basically contains
/// the cache index (protected by a mutex) and a weak self reference to itself.
pub struct Cache<K: CacheKey, V: Cacheable<K>> {
    index: Mutex<CacheIndex<K, V>>,
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
}

pub(super) type INodeCacheKey = (usize, usize);
pub(super) type INodeCache = Cache<INodeCacheKey, CachedINode>;

pub(super) struct CachedINode(Arc<dyn INodeInterface>);

impl Cacheable<INodeCacheKey> for CachedINode {}

/// Clears the inode cache. This function is mostly called when cleanup is required. For example
/// on a reboot or shutdown.
pub fn clear_inode_cache() {
    if let Some(cache) = INODE_CACHE.get() {
        cache.clear();
    }
}

/// This function is responsible for initializing the inode cache.
pub fn init() {
    INODE_CACHE.call_once(|| INodeCache::new(256));
}
