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

use alloc::{collections::BTreeMap, string::String, sync::Arc};

use spin::{Once, RwLock};

use self::{cache::DirCacheItem, ramfs::RamFs};

pub mod cache;
pub mod dev_fs;
pub mod file_table;
pub mod inode;
pub mod ramfs;

static FILE_SYSTEMS: RwLock<BTreeMap<usize, Arc<dyn FileSystem>>> = RwLock::new(BTreeMap::new());

type Result<T> = core::result::Result<T, FilesystemError>;

/// ## Notes
/// * https://wiki.osdev.org/File_Systems
pub trait FileSystem: Send + Sync {
    fn root_dir(&self) -> DirCacheItem {
        todo!()
    }
}

#[inline(always)]
pub(super) fn install_filesystem<F: 'static + FileSystem>(
    signature: usize,
    filesystem: F,
) -> Result<()> {
    let fs = FILE_SYSTEMS.read();

    if fs.contains_key(&signature) {
        Err(FilesystemError::DeviceExists)
    } else {
        drop(fs);
        FILE_SYSTEMS.write().insert(signature, Arc::new(filesystem));

        Ok(())
    }
}

#[derive(Debug)]
pub enum FilesystemError {
    DeviceExists,
    NotSupported,
    EntryExists,
    EntryNotFound,
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

static ROOT_FS: Once<Arc<RamFs>> = Once::new();
static ROOT_DIR: Once<DirCacheItem> = Once::new();

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
                    continue;
                }

                result = result.inode().lookup(result.clone(), component)?;
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
    lookup_path(Path::new("/dev"))?;

    dev_fs::init().expect("Failed to initialize devfs");

    Ok(())
}
