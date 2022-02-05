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

use core::sync::atomic::{AtomicUsize, Ordering};

use aero_syscall::MMapFlags;
use aero_syscall::{OpenFlags, SocketAddr};

use alloc::string::String;
use alloc::sync::Arc;
use alloc::sync::Weak;

use alloc::vec::Vec;
use spin::Once;

use crate::mem::paging::PhysAddr;
use crate::utils::sync::Mutex;
use crate::utils::Downcastable;

use super::cache;
use super::cache::Cacheable;
use super::cache::CachedINode;
use super::cache::{DirCacheItem, INodeCacheItem};
use super::devfs::DevINode;
use super::{FileSystem, FileSystemError, Result};

static DIR_CACHE_MARKER: AtomicUsize = AtomicUsize::new(0x00);

/// An inode describes a file. An inode structure holds metadata of the
/// inode which includes its type, size, the number of links referring to it,
/// and the list of blocks holding the file's content. For example device files,
/// files on the disk, etc...
///
/// This trait requires the implementor to implement [Send], [Sync] and [Downcastable] on
/// the inode structure.
pub trait INodeInterface: Send + Sync + Downcastable {
    /// Returns the inode metadata of `this` inode.
    fn metadata(&self) -> Result<Metadata> {
        Err(FileSystemError::NotSupported)
    }

    /// Read at the provided `offset` to the given `buffer`.
    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> Result<usize> {
        Err(FileSystemError::NotSupported)
    }

    /// Write at the provided `offset` with the given `buffer` as its contents.
    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> Result<usize> {
        Err(FileSystemError::NotSupported)
    }

    /// Creates a new directory with the provided `name` in the filesystem.
    fn mkdir(&self, _name: &str) -> Result<INodeCacheItem> {
        Err(FileSystemError::NotSupported)
    }

    fn rmdir(&self, _name: &str) -> Result<()> {
        Err(FileSystemError::NotSupported)
    }

    /// Creates a new file with the provoded `name` in the filesystem.
    fn touch(&self, _parent: DirCacheItem, _name: &str) -> Result<DirCacheItem> {
        Err(FileSystemError::NotSupported)
    }

    /// Creates a new dev inode with the provided `name` and the device `marker` in
    /// the filesystem.
    ///
    /// ## Overview
    /// In the inner implementation this simply looks up for the device with the device
    /// marker in the global devices b-tree map and adds it as a device inode in the children
    /// array of itself.
    fn make_dev_inode(&self, _name: &str, _marker: usize) -> Result<INodeCacheItem> {
        Err(FileSystemError::NotSupported)
    }

    fn make_ramfs_inode(&self, _name: &str, _buffer: &'static [u8]) -> Result<INodeCacheItem> {
        Err(FileSystemError::NotSupported)
    }

    fn make_local_socket_inode(
        &self,
        _name: &str,
        _inode: Arc<dyn INodeInterface>,
    ) -> Result<INodeCacheItem> {
        Err(FileSystemError::NotSupported)
    }

    /// Looks up the directory entry in the filesystem.
    fn lookup(&self, _dir: DirCacheItem, _name: &str) -> Result<DirCacheItem> {
        Err(FileSystemError::NotSupported)
    }

    fn open(&self, _flags: OpenFlags) -> Result<()> {
        Ok(())
    }

    fn close(&self, _flags: OpenFlags) {}

    fn dirent(&self, _parent: DirCacheItem, _index: usize) -> Result<Option<DirCacheItem>> {
        Err(FileSystemError::NotSupported)
    }

    /// Returns a weak reference to the filesystem that this inode belongs to.
    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        None
    }

    fn ioctl(&self, _command: usize, _arg: usize) -> Result<usize> {
        Err(FileSystemError::NotSupported)
    }

    fn truncate(&self, _size: usize) -> Result<()> {
        Err(FileSystemError::NotSupported)
    }

    /// ## Saftey
    ///
    /// The caller is responsible for removing the inode from the cache.
    fn unlink(&self, _name: &str) -> Result<()> {
        Err(FileSystemError::NotSupported)
    }

    fn mmap(&self, _offset: usize, _flags: MMapFlags) -> Result<PhysAddr> {
        Err(FileSystemError::NotSupported)
    }

    // Socket operations
    fn bind(&self, _address: &SocketAddr, _length: usize) -> Result<()> {
        Err(FileSystemError::NotSupported)
    }
}

/// Structure representing the curcial, characteristics of an inode. The metadata
/// of an inode can be retrieved by invoking the [INodeInterface::metadata] function.
#[derive(Debug, Copy, Clone)]
pub struct Metadata {
    pub id: usize,
    pub file_type: FileType,

    /// The total size of the content that the inode holds. Set to `0x00` if
    /// the inode file type is *not* a file.
    pub size: usize,

    /// The length of the children's map of the inode. Set to `0x00` if the inode
    /// has no children and if the file type of the inode is *not* a directory.
    pub children_len: usize,
}

impl Metadata {
    #[inline]
    pub fn id(&self) -> usize {
        self.id
    }

    #[inline]
    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    /// Returns [`true`] if the file type of the inode is a file.
    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File
    }

    /// Returns [`true`] if the file type of the inode is a directory.
    #[inline]
    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Directory
    }

    /// Returns [`true`] if the file type of the inode is a socket.
    #[inline]
    pub fn is_socket(&self) -> bool {
        self.file_type == FileType::Socket
    }
}

/// Enum representing the inner contents of a file. The file contents depend on the
/// file type of the inode.
pub enum FileContents {
    /// This variant expresses a *normal file* (akin: A file that actually stores data
    /// in bytes) and is protected by a spin lock.
    Content(Mutex<Vec<u8>>),

    /// This variant is similar to the one above, except it's read only
    /// and is backed by a static byte buffer
    StaticContent(&'static [u8]),

    /// If the file type of the inode is [FileType::Device], in that case this variant
    /// is used.
    Device(Arc<DevINode>),

    /// This variant is used to store the backing socket inode.
    Socket(Arc<dyn INodeInterface>),

    /// This file does *not* and *cannot* have any contents in bytes. This is useful
    /// in the cases of directories.
    None,
}

impl Default for FileContents {
    fn default() -> Self {
        Self::Content(Mutex::new(Vec::new()))
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FileType {
    File,
    Directory,
    Device,
    Socket,
}

impl From<FileType> for aero_syscall::SysFileType {
    fn from(file: FileType) -> Self {
        match file {
            FileType::File => aero_syscall::SysFileType::File,
            FileType::Directory => aero_syscall::SysFileType::Directory,
            FileType::Device => aero_syscall::SysFileType::Device,
            FileType::Socket => aero_syscall::SysFileType::Socket,
        }
    }
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

/// A directory entry is basically the mapping of filename to its inode.
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

    pub fn from_inode(inode: Arc<dyn INodeInterface>) -> DirCacheItem {
        let icache = cache::icache();
        let inode = icache.make_item_no_cache(CachedINode::new(inode));

        cache::dcache().make_item_no_cache(Self {
            data: Mutex::new(DirProtectedData {
                parent: None,

                name: String::new(),
                inode: inode.clone(),
            }),

            filesystem: if let Some(fs) = inode.weak_filesystem() {
                Once::initialized(fs)
            } else {
                Once::new()
            },

            cache_marker: 0,
        })
    }

    pub fn from_socket_inode(
        parent: DirCacheItem,
        name: String,
        inode: Arc<dyn INodeInterface>,
    ) -> Result<DirCacheItem> {
        let inode = parent
            .inode()
            .make_local_socket_inode(name.as_str(), inode)?;

        Ok(cache::dcache().make_item_no_cache(Self {
            data: Mutex::new(DirProtectedData {
                parent: Some(parent),
                inode: inode.clone(),
                name,
            }),
            filesystem: if let Some(filesystem) = inode.weak_filesystem() {
                Once::initialized(filesystem)
            } else {
                Once::new()
            },
            cache_marker: 0,
        }))
    }

    pub fn name(&self) -> String {
        self.data.lock().name.clone()
    }

    /// Returns the inner cached inode item of the directory entry.
    pub fn inode(&self) -> INodeCacheItem {
        self.data.lock().inode.clone()
    }

    pub fn parent(&self) -> Option<DirCacheItem> {
        self.data.lock().parent.clone()
    }

    /// Drops the directory entry from the cache.
    pub fn drop_from_cache(&self) {
        cache::dcache().remove(&self.cache_key());
        cache::icache().remove(&self.inode().cache_key());
    }
}

/// Fetches a cached directory entry item from the directory cache. Returns if
/// the provided entry exists in the given parent directory cache.
pub fn fetch_dir_entry(parent: DirCacheItem, name: String) -> Option<DirCacheItem> {
    let dcache = cache::dcache();
    let cache_key = (parent.cache_marker, name);

    dcache.get(cache_key)
}
