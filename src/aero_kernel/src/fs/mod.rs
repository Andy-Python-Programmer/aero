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

use core::mem;

use aero_syscall::AeroSyscallError;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;

use spin::Mutex;
use spin::Once;

use crate::fs::inode::FileType;

use self::cache::Cacheable;
use self::{cache::DirCacheItem, ramfs::RamFs};

pub mod cache;
pub mod devfs;
pub mod file_table;
pub mod inode;
pub mod ramfs;

static ROOT_FS: Once<Arc<RamFs>> = Once::new();
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

    fn mount(&self, directory: DirCacheItem, filesystem: Arc<dyn FileSystem>) -> Result<()> {
        let mut this = self.0.lock();
        let mount_key = directory.cache_key();

        if this.contains_key(&mount_key) {
            return Err(FileSystemError::EntryExists);
        }

        let root_dir = filesystem.root_dir();

        let current_data = directory.data.lock();
        let mut root_data = root_dir.data.lock();

        root_data.parent = current_data.parent.clone();

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

    fn find_mount(&self, directory: DirCacheItem) -> Result<MountPoint> {
        let this = self.0.lock();
        let cache_key = directory.cache_key();

        if let Some(mount_point) = this.get(&cache_key) {
            Ok(mount_point.clone())
        } else {
            Err(FileSystemError::EntryNotFound)
        }
    }
}

/// ## Notes
/// * https://wiki.osdev.org/File_Systems
pub trait FileSystem: Send + Sync {
    fn root_dir(&self) -> DirCacheItem {
        todo!()
    }
}

#[derive(Debug)]
pub enum FileSystemError {
    NotSupported,
    EntryExists,
    EntryNotFound,
}

impl From<FileSystemError> for AeroSyscallError {
    fn from(error: FileSystemError) -> Self {
        match error {
            FileSystemError::NotSupported => AeroSyscallError::EACCES,
            FileSystemError::EntryExists => AeroSyscallError::EEXIST,
            FileSystemError::EntryNotFound => AeroSyscallError::ENOENT,
        }
    }
}

/// A slice of a path (akin to [str]).
#[derive(Debug)]
pub struct Path(str);

impl Path {
    pub fn new(path: &str) -> &Self {
        unsafe { &*(path as *const str as *const Path) }
    }

    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split("/").filter(|e| *e != "" && *e != ".")
    }
}

pub fn lookup_path(path: &Path) -> Result<DirCacheItem> {
    let mut result = root_dir().clone();

    // Iterate and resolve each component. For example `a`, `b`, and `c` in `a/b/c`.
    for component in path.components() {
        match component {
            // Handle some special cases that might occur in a relative path.
            "." => continue,
            ".." => {}

            _ => {
                /*
                 * After we have resolved all of the special cases that might occur in a path, now
                 * we have to resolve the directory entry itself. For example `a` in `./a/` and since this
                 * is recursive we resolve the next component also.
                 */
                let cache_entry = inode::fetch_dir_entry(result.clone(), String::from(component));

                if let Some(entry) = cache_entry {
                    result = entry;
                } else {
                    result = result.inode().lookup(result.clone(), component)?;
                }

                if result.inode().metadata().file_type == FileType::Directory {
                    if let Ok(mount_point) = MOUNT_MANAGER.find_mount(result.clone()) {
                        result = mount_point.root_entry;
                    }
                }
            }
        }
    }

    Ok(result)
}

pub fn root_dir() -> &'static DirCacheItem {
    ROOT_DIR.get().expect("How's this possible?")
}

pub fn init() -> Result<()> {
    cache::init();

    let filesystem = RamFs::new();

    ROOT_FS.call_once(|| filesystem.clone());
    ROOT_DIR.call_once(|| filesystem.root_dir().clone());

    root_dir().inode().mkdir("dev")?;
    root_dir().inode().mkdir("etc")?;
    root_dir().inode().mkdir("home")?;
    root_dir().inode().mkdir("temp")?;

    devfs::init()?;
    log::info!("Installed devfs");

    Ok(())
}
