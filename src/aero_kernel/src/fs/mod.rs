// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use core::mem;

pub mod path;

// TODO: Do not re-export this.
pub use path::Path;

use aero_syscall::SyscallError;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use crate::fs::cache::DirCacheImpl;
use crate::userland::scheduler;
use crate::utils::sync::Mutex;
use spin::Once;

use self::cache::{Cacheable, DirCacheItem};

pub mod block;
pub mod cache;
pub mod devfs;
pub mod epoll;
pub mod eventfd;
pub mod ext2;
pub mod file_table;
pub mod inode;
pub mod pipe;
pub mod procfs;
pub mod ramfs;

static ROOT_FS: Once<Arc<dyn FileSystem>> = Once::new();
static ROOT_DIR: Once<DirCacheItem> = Once::new();

lazy_static::lazy_static! {
    pub static ref MOUNT_MANAGER: MountManager = MountManager::new();
}

pub type Result<T> = core::result::Result<T, FileSystemError>;
type MountKey = (usize, String);

#[derive(Clone)]
struct MountPoint {
    filesystem: Arc<dyn FileSystem>,

    root_entry: DirCacheItem,
    origin_entry: DirCacheItem,
}

#[repr(transparent)]
pub struct MountManager(Mutex<BTreeMap<MountKey, MountPoint>>);

impl MountManager {
    #[inline]
    fn new() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }

    pub fn mount(&self, directory: DirCacheItem, filesystem: Arc<dyn FileSystem>) -> Result<()> {
        let mut this = self.0.lock();
        let mount_key = directory.cache_key();

        if this.contains_key(&mount_key) {
            return Err(FileSystemError::EntryExists);
        }

        let root_dir = filesystem.root_dir();

        let current_data = directory.data.lock();
        let mut root_data = root_dir.data.lock();

        root_data.name.clone_from(&current_data.name);
        root_data.parent.clone_from(&current_data.parent);

        mem::drop(root_data);
        mem::drop(current_data);

        this.insert(
            mount_key,
            MountPoint {
                filesystem,
                root_entry: root_dir,
                origin_entry: directory,
            },
        );

        Ok(())
    }

    fn find_mount(&self, dir: &DirCacheItem) -> Result<MountPoint> {
        let this = self.0.lock();
        let cache_key = dir.cache_key();

        if let Some(mount_point) = this.get(&cache_key) {
            Ok(mount_point.clone())
        } else {
            Err(FileSystemError::EntryNotFound)
        }
    }
}

pub trait FileSystem: Send + Sync {
    fn root_dir(&self) -> DirCacheItem {
        todo!()
    }
}

#[derive(Debug, PartialEq)]
pub enum FileSystemError {
    NotSupported,
    EntryExists,
    EntryNotFound,
    Busy,
    NotDirectory,
    IsPipe,
    IsDir,
    Interrupted,
    TooSmall,
    InvalidPath,
    NotSocket,
    ConnectionRefused,
    NotConnected,
    WouldBlock,
    NoTty,
}

impl From<FileSystemError> for SyscallError {
    fn from(error: FileSystemError) -> Self {
        match error {
            FileSystemError::NotSupported => Self::EACCES,
            FileSystemError::EntryExists => Self::EEXIST,
            FileSystemError::EntryNotFound => Self::ENOENT,
            FileSystemError::Busy => Self::EBUSY,
            FileSystemError::NotDirectory => Self::ENOTDIR,
            FileSystemError::IsPipe => Self::ESPIPE,
            FileSystemError::Interrupted => Self::EINTR,
            FileSystemError::TooSmall => Self::E2BIG,
            FileSystemError::InvalidPath => Self::EINVAL,
            FileSystemError::NotSocket => Self::ENOTSOCK,
            FileSystemError::ConnectionRefused => Self::ECONNREFUSED,
            FileSystemError::IsDir => Self::EISDIR,
            FileSystemError::NotConnected => Self::ENOTCONN,
            FileSystemError::WouldBlock => Self::EAGAIN,
            FileSystemError::NoTty => Self::ENOTTY,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LookupMode {
    None,
    /// Creates the file if it does not exist.
    Create,
}

pub fn lookup_path_with(
    mut cwd: DirCacheItem,
    path: &Path,
    mode: LookupMode,
    resolve_last: bool,
) -> Result<DirCacheItem> {
    let components_len = path.components().count();

    // Iterate and resolve each component. For example `a`, `b`, and `c` in `a/b/c`.
    for (i, component) in path.components().enumerate() {
        match component {
            // Handle some special cases that might occur in a relative path.
            "." => continue,
            ".." => {
                let current = cwd.data.lock();

                if let Some(parent) = current.parent.clone() {
                    core::mem::drop(current); // drop the data lock.
                    cwd = parent;
                }

                // Else the entry does not have a parent, ie. the current entry is the root and
                // we can't go any further :^)
            }

            _ => {
                // After we have resolved all of the special cases that might occur in a path, now
                // we have to resolve the directory entry itself. For example `a` in `./a/`.
                let cache_entry = inode::fetch_dir_entry(&cwd, String::from(component));
                let parent = cwd.clone();

                if let Some(entry) = cache_entry {
                    cwd = entry;
                } else {
                    match cwd.inode().lookup(cwd.clone(), component) {
                        Ok(entry) => cwd = entry,

                        Err(err)
                            if err == FileSystemError::EntryNotFound
                                && mode == LookupMode::Create =>
                        {
                            if i == components_len - 1 {
                                cwd = cwd.inode().touch(cwd.clone(), component)?;
                            } else {
                                // todo: fix this shit
                                cwd.inode().mkdir(component)?;
                                cwd = match lookup_path_with(
                                    cwd.clone(),
                                    Path::new(component),
                                    LookupMode::None,
                                    resolve_last,
                                ) {
                                    Ok(x) => x,
                                    Err(e) => {
                                        dbg!(component, cwd.absolute_path());
                                        return Err(dbg!(e));
                                    }
                                };
                            }
                        }

                        Err(err) => return Err(err),
                    }
                }

                let inode = cwd.inode();
                let metadata = inode.metadata()?;

                if metadata.is_symlink() && resolve_last {
                    let resolved_path = inode.resolve_link()?;

                    cwd = lookup_path_with(
                        if resolved_path.is_absolute() {
                            root_dir().clone()
                        } else {
                            parent
                        },
                        resolved_path.as_ref(),
                        LookupMode::None,
                        resolve_last,
                    )?;
                } else if metadata.is_directory() {
                    if let Ok(mount_point) = MOUNT_MANAGER.find_mount(&cwd) {
                        cwd = mount_point.root_entry;
                    }
                }
            }
        }
    }

    Ok(cwd)
}

pub fn lookup_path(path: &Path) -> Result<DirCacheItem> {
    let cwd = if !path.is_absolute() {
        scheduler::current_thread().cwd_dirent()
    } else {
        root_dir().clone()
    };

    // TODO:Keep `resolve_last` set to true as a default?
    lookup_path_with(cwd, path, LookupMode::None, true)
}

pub fn root_dir() -> &'static DirCacheItem {
    ROOT_DIR.get().expect("How's this possible?")
}

pub fn init() -> Result<()> {
    cache::init();
    Ok(())
}
