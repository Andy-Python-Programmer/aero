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

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::string::String;
use alloc::sync::Weak;

use spin::{Mutex, Once};

use crate::utils::Downcastable;

use super::cache;
use super::cache::{DirCacheItem, INodeCacheItem};
use super::{FileSystem, FilesystemError, Result};

static DIR_CACHE_MARKER: AtomicUsize = AtomicUsize::new(0x00);

/// An inode describes a file. An inode structure holds metadata of the
/// inode which includes its type, size, the number of links referring to it,
/// and the list of blocks holding the file's content. For example device files,
/// files on the disk, etc...
///
/// This trait requires the implementor to implement [Send], [Sync] and [Downcastable] on
/// the inode structure.
pub trait INodeInterface: Send + Sync + Downcastable {
    /// Write at the provided `offset` with the given `buffer` as its contents.
    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> Result<usize> {
        Err(FilesystemError::NotSupported)
    }

    /// Read at the provided `offset` to the given `buffer.
    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> Result<usize> {
        Err(FilesystemError::NotSupported)
    }

    /// Creates a new directory with the provided `name` in the filesystem.
    fn mkdir(&self, _name: &str) -> Result<INodeCacheItem> {
        Err(FilesystemError::NotSupported)
    }

    /// Looks up the directory entry in the filesystem.
    fn lookup(&self, _dir: DirCacheItem, _name: &str) -> Result<DirCacheItem> {
        Err(FilesystemError::NotSupported)
    }

    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        None
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum FileType {
    File,
    Directory,
}

impl Default for FileType {
    fn default() -> Self {
        Self::File
    }
}

pub(super) struct DirProtectedData {
    pub(super) name: String,
    pub(super) parent: Option<DirCacheItem>,

    inode: INodeCacheItem,
}

pub struct DirEntry {
    pub(super) data: Mutex<DirProtectedData>,
    pub(super) filesystem: Once<Weak<dyn FileSystem>>,
    pub(super) cache_marker: usize,
}

impl DirEntry {
    /// Creates a new cached directory entry, where the entry has the provided `parent` and
    /// uses the weak filesystem pointer that the provided `inode` holds.
    pub fn new(parent: DirCacheItem, inode: INodeCacheItem, name: String) -> DirCacheItem {
        let dcache = cache::dcache();

        /*
         * Helper bool to avoid situations where the directory entry is already cached. The possible
         * cases are:
         *
         * "." (ie. we do not want to re-cache the current directory)
         * ".." (ie. we do not want to re-cache the current directory's, parent directory).
         */
        let cache_me = ![".", ".."].contains(&name.as_str());

        let entry = Self {
            data: Mutex::new(DirProtectedData {
                parent: Some(parent.clone()),
                inode: inode.clone(),
                name,
            }),

            cache_marker: if cache_me {
                DIR_CACHE_MARKER.fetch_add(1, Ordering::SeqCst)
            } else {
                0x00
            },

            filesystem: if let Some(filesystem) = inode.weak_filesystem() {
                Once::initialized(filesystem)
            } else {
                Once::new()
            },
        };

        if cache_me {
            dcache.make_item_cached(entry)
        } else {
            dcache.make_item_no_cache(entry)
        }
    }

    /// Creates a new root cached directory entry where the there is no parent
    /// of the cache item and no filesystem reference by default. The caller is responsible
    /// for initializing the weak reference to the filesystem.
    pub fn new_root(inode: INodeCacheItem, name: String) -> DirCacheItem {
        let dcache = cache::dcache();

        dcache.make_item_no_cache(Self {
            data: Mutex::new(DirProtectedData {
                parent: None,
                inode: inode.clone(),
                name,
            }),

            cache_marker: DIR_CACHE_MARKER.fetch_add(1, Ordering::SeqCst),
            filesystem: Once::new(),
        })
    }

    /// Returns the inner inode cache item of the directory entry cache.
    pub fn inode(&self) -> INodeCacheItem {
        self.data.lock().inode.clone()
    }
}

/// Fetches a cached directory entry item from the directory cache. Returns if
/// the provided entry exists in the given parent directory cache.
pub fn fetch_dir_entry(parent: DirCacheItem, name: String) -> Option<DirCacheItem> {
    let dcache = cache::dcache();
    let cache_key = (parent.cache_marker, name.clone());

    dcache.get(cache_key)
}
