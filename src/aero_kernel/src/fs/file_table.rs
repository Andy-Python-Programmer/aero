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

use core::sync::atomic::{AtomicUsize, Ordering};

use aero_syscall::OpenFlags;

use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::RwLock;

use super::cache::DirCacheItem;
use super::FileSystemError;

pub struct FileHandle {
    pub fd: usize,
    pub inode: DirCacheItem,
    pub offset: AtomicUsize,
    pub flags: OpenFlags,
}

impl FileHandle {
    /// Creates a new file handle.
    #[inline]
    pub fn new(fd: usize, inode: DirCacheItem, flags: OpenFlags) -> Self {
        Self {
            fd,
            inode: inode.clone(),
            offset: AtomicUsize::new(0),
            flags,
        }
    }

    pub fn read(&self, buffer: &mut [u8]) -> super::Result<usize> {
        let offset = self.offset.load(Ordering::SeqCst);
        let new_offset = self.inode.inode().read_at(offset, buffer)?;

        self.offset.fetch_add(new_offset, Ordering::SeqCst);

        Ok(new_offset)
    }

    pub fn write(&self, buffer: &[u8]) -> super::Result<usize> {
        let offset = self.offset.load(Ordering::SeqCst);
        let new_offset = self.inode.inode().write_at(offset, buffer)?;

        self.offset.fetch_add(new_offset, Ordering::SeqCst);

        Ok(new_offset)
    }
}

#[repr(transparent)]
pub struct FileTable(RwLock<Vec<Option<Arc<FileHandle>>>>);

impl FileTable {
    pub fn new() -> Self {
        let mut table = Vec::new();
        table.resize(256, None);

        Self(RwLock::new(table))
    }

    pub fn get_handle(&self, fd: usize) -> Option<Arc<FileHandle>> {
        let files = self.0.read();

        if let Some(file) = &files.get(fd) {
            if let Some(handle) = file {
                return Some(handle.clone());
            }
        }

        None
    }

    pub fn open_file(&self, dentry: DirCacheItem, mut flags: OpenFlags) -> super::Result<usize> {
        let mut files = self.0.write();

        // Remove all of the unneccessary flags.
        flags.remove(OpenFlags::O_CREAT);
        flags.remove(OpenFlags::O_DIRECTORY);

        // Check if a file handle was removed, if so re-use the file handle.
        if let Some((i, f)) = files.iter_mut().enumerate().find(|e| e.1.is_none()) {
            let handle = Arc::new(FileHandle::new(i, dentry, flags));

            handle.inode.inode().open(flags)?;
            *f = Some(handle);

            Ok(i)
        } else if files.len() < 256 {
            let fd = files.len();
            let handle = Arc::new(FileHandle::new(fd, dentry, flags));

            handle.inode.inode().open(flags)?;
            files.push(Some(handle));

            Ok(fd)
        } else {
            Err(FileSystemError::Busy)
        }
    }
}
