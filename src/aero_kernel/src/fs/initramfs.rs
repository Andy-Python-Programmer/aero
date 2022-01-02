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

use alloc::sync::Arc;

use crate::fs::{FileSystemError, Path};

use super::cache::DirCacheItem;
use super::ramfs::RamFs;

use super::{root_dir, FileSystem, Result, MOUNT_MANAGER};

lazy_static::lazy_static! {
    static ref INIT_FILESYSTEM: Arc<InitRamFs> = InitRamFs::new();
}

struct InitRamFs(Arc<RamFs>);

impl InitRamFs {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(RamFs::new()))
    }
}

impl FileSystem for InitRamFs {
    fn root_dir(&self) -> DirCacheItem {
        self.0.root_dir()
    }
}

pub(super) fn init() -> Result<()> {
    lazy_static::initialize(&INIT_FILESYSTEM);

    let initrd_module = crate::INITRD_MODULE.get().unwrap();
    let initrd = unsafe {
        let base = crate::PHYSICAL_MEMORY_OFFSET + initrd_module.start;
        let length = initrd_module.end - initrd_module.start;

        core::slice::from_raw_parts(base.as_ptr(), length as usize)
    };

    for entry in cpio_reader::iter_files(initrd) {
        let path = Path::new(entry.name());
        let component_count = path.components().count();

        let mut cwd = root_dir().clone();

        for (i, component) in path.components().enumerate() {
            if i == component_count - 1 {
                cwd.inode().make_ramfs_inode(component, entry.file())?;
            } else {
                match cwd.inode().lookup(cwd.clone(), component) {
                    Ok(new_cwd) => cwd = new_cwd,
                    Err(FileSystemError::EntryNotFound) => {
                        cwd.inode().mkdir(component)?;
                        cwd = cwd.inode().lookup(cwd.clone(), component)?;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }

    MOUNT_MANAGER.mount(root_dir().clone(), INIT_FILESYSTEM.clone())?;

    Ok(())
}
