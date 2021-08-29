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
use stivale_boot::v2::StivaleModuleTag;

use crate::mem::paging::*;
use crate::utils::sync::Mutex;
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
    Busy,
    NotDirectory,
}

impl From<FileSystemError> for AeroSyscallError {
    fn from(error: FileSystemError) -> Self {
        match error {
            FileSystemError::NotSupported => AeroSyscallError::EACCES,
            FileSystemError::EntryExists => AeroSyscallError::EEXIST,
            FileSystemError::EntryNotFound => AeroSyscallError::ENOENT,
            FileSystemError::Busy => AeroSyscallError::EBUSY,
            FileSystemError::NotDirectory => AeroSyscallError::ENOTDIR,
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

                if result.inode().metadata()?.file_type == FileType::Directory {
                    if let Ok(mount_point) = MOUNT_MANAGER.find_mount(result.clone()) {
                        result = mount_point.root_entry;
                    }
                }
            }
        }
    }

    Ok(result)
}

/// Representation of the header of an entry in an archive.
#[repr(C)]
struct UstarHeader {
    name: [u8; 100],
    mode: [u8; 8],
    uid: [u8; 8],
    gid: [u8; 8],
    size: [u8; 12],
    mtime: [u8; 12],
    cksum: [u8; 8],
    typeflag: [u8; 1],
    linkname: [u8; 100],

    /* USTAR format */
    magic: [u8; 6],
    version: [u8; 2],
    uname: [u8; 32],
    gname: [u8; 32],
    dev_major: [u8; 8],
    dev_minor: [u8; 8],
    prefix: [u8; 155],
}

#[repr(u8)]
enum UstarFileType {
    File = 0x30,
    HardLink = 0x31,
    SymLink = 0x32,
    CharDevice = 0x33,
    BlockDevice = 0x34,
    Directory = 0x35,
    Fifo = 0x36,
}

pub fn root_dir() -> &'static DirCacheItem {
    ROOT_DIR.get().expect("How's this possible?")
}

pub fn init(modules: &'static StivaleModuleTag) -> Result<()> {
    cache::init();

    let filesystem = RamFs::new();

    ROOT_FS.call_once(|| filesystem.clone());
    ROOT_DIR.call_once(|| filesystem.root_dir().clone());

    for module in modules.iter() {
        // Note: Loading initramfs may be slower if running with legacy BIOS since it loads
        // the archive in 64K chunks and has to switch back to real mode each time.
        if module.as_str() == "initramfs" {
            log::info!(
                "initramfs: unpacking at (address={:#x}, size={:#x})...",
                module.start,
                module.size()
            );

            let stream = unsafe {
                let addr = crate::PHYSICAL_MEMORY_OFFSET + module.start;
                core::slice::from_raw_parts::<u8>(addr.as_ptr(), module.size() as usize)
            };

            let mut archive_index = 0;

            while let Some(header) = stream.get(archive_index..).and_then(|s| {
                let header = unsafe { &*(s.as_ptr() as *mut UstarHeader) };

                // NOTE: The magic can either end with a ASCII space character or a NULL byte so,
                // we check the first 5 bits of the magic instead.
                if &header.magic[0..5] != b"ustar" || s.len() < 512 {
                    None
                } else {
                    Some(header)
                }
            }) {
                let path = unsafe { core::str::from_utf8_unchecked(&header.name) };
                let file_type = match header.typeflag[0] {
                    0x30 => Ok(UstarFileType::File),
                    0x31 => Ok(UstarFileType::HardLink),
                    0x32 => Ok(UstarFileType::SymLink),
                    0x33 => Ok(UstarFileType::CharDevice),
                    0x34 => Ok(UstarFileType::BlockDevice),
                    0x35 => Ok(UstarFileType::Directory),
                    0x36 => Ok(UstarFileType::Fifo),
                    _ => Err(FileSystemError::NotSupported),
                }?;

                let size = usize::from_str_radix(
                    unsafe { core::str::from_utf8_unchecked(&header.size) },
                    8,
                )
                .unwrap_or(0);

                match file_type {
                    UstarFileType::File => {}
                    UstarFileType::Directory => {
                        // The root directory is automatically created in the constructor of the filesystem.
                        if path != "./" {
                            root_dir().inode().mkdir(path)?;
                        }
                    }

                    _ => (),
                }

                // TODO: Free the memory allocated for the ustar header.
                archive_index += 0x200 + align_up(size as u64, 512) as usize;
            }
        }
    }

    root_dir().inode().mkdir("dev")?;
    root_dir().inode().mkdir("etc")?;
    root_dir().inode().mkdir("home")?;
    root_dir().inode().mkdir("temp")?;

    devfs::init()?;
    log::info!("Installed devfs");

    Ok(())
}
