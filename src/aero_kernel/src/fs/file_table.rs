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

use aero_syscall::{OpenFlags, SysDirEntry};

use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::RwLock;

use super::cache::{DirCacheItem, INodeCacheItem};
use super::inode::FileType;
use super::FileSystemError;

pub struct FileHandle {
    pub fd: usize,
    pub inode: DirCacheItem,
    // We need to store the `offset` behind an Arc since when the file handle
    // is duplicated, the `offset` needs to be in sync with the parent.
    pub offset: Arc<AtomicUsize>,
    pub flags: OpenFlags,
}

impl FileHandle {
    /// Creates a new file handle.
    pub fn new(fd: usize, inode: DirCacheItem, flags: OpenFlags) -> Self {
        Self {
            fd,
            inode: inode.clone(),
            offset: Arc::new(AtomicUsize::new(0)),
            flags,
        }
    }

    pub fn read(&self, buffer: &mut [u8]) -> super::Result<usize> {
        let offset = self.offset.load(Ordering::SeqCst);
        let new_offset = self.inode.inode().read_at(offset, buffer)?;

        self.offset.fetch_add(new_offset, Ordering::SeqCst);

        Ok(new_offset)
    }

    pub fn seek(&self, off: isize, whence: aero_syscall::SeekWhence) -> super::Result<usize> {
        let meta = self
            .inode
            .inode()
            .metadata()
            .ok()
            .ok_or(FileSystemError::IsPipe)?;

        if meta.file_type() == FileType::File {
            match whence {
                aero_syscall::SeekWhence::SeekSet => {
                    self.offset.store(off as usize, Ordering::SeqCst);
                }

                aero_syscall::SeekWhence::SeekCur => {
                    let mut offset = self.offset.load(Ordering::SeqCst) as isize;
                    offset += off;

                    self.offset.store(offset as usize, Ordering::SeqCst);
                }

                aero_syscall::SeekWhence::SeekEnd => {
                    let mut offset = meta.size as isize;
                    offset += off;

                    self.offset.store(offset as usize, Ordering::SeqCst);
                }
            }

            Ok(self.offset.load(Ordering::SeqCst))
        } else {
            Err(FileSystemError::IsPipe)
        }
    }

    pub fn write(&self, buffer: &[u8]) -> super::Result<usize> {
        let offset = self.offset.load(Ordering::SeqCst);
        let new_offset = self.inode.inode().write_at(offset, buffer)?;

        self.offset.fetch_add(new_offset, Ordering::SeqCst);

        Ok(new_offset)
    }

    pub fn inode(&self) -> INodeCacheItem {
        self.inode.inode()
    }

    pub fn duplicate(&self, flags: OpenFlags) -> super::Result<Arc<FileHandle>> {
        let flags = self.flags | flags;
        let new = Arc::new(Self {
            fd: self.fd,
            inode: self.inode.clone(),
            offset: self.offset.clone(),
            flags,
        });

        new.inode.inode().open(flags)?;

        Ok(new)
    }

    pub fn get_dents(&self, mut buffer: &mut [u8]) -> super::Result<usize> {
        let mut offset = 0x00usize;

        loop {
            let inode = self
                .inode
                .inode()
                .dirent(self.inode.clone(), self.offset.load(Ordering::SeqCst))?;

            if let Some(entry) = inode {
                let reclen = core::mem::size_of::<SysDirEntry>() + entry.name().len();
                let dir_offset = offset + reclen;

                let file_type = entry.inode().metadata()?.file_type();
                let file_type: aero_syscall::SysFileType = file_type.into();

                let sysd = SysDirEntry {
                    inode: entry.inode().metadata()?.id(),
                    offset: dir_offset,
                    reclen,
                    file_type: file_type as usize,
                    name: [], // will be filled in later
                };

                if buffer.len() < sysd.reclen {
                    break Ok(offset);
                }

                self.offset.fetch_add(1, Ordering::SeqCst);

                unsafe {
                    let sysd_ref = &mut *(buffer.as_mut_ptr() as *mut SysDirEntry);

                    // Copy the directory entry info into the provided buffer.
                    buffer.as_mut_ptr().copy_from(
                        &sysd as *const _ as *const u8,
                        core::mem::size_of::<SysDirEntry>(),
                    );

                    // Copy over the name of the inode.
                    sysd_ref
                        .name
                        .as_mut_ptr()
                        .copy_from(entry.name().as_ptr(), entry.name().len());
                }

                offset += sysd.reclen;
                buffer = &mut buffer[sysd.reclen..];
            } else {
                break Ok(offset);
            }
        }
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

    pub fn duplicate(
        &self,
        fd: usize,
        flags: OpenFlags,
    ) -> Result<usize, aero_syscall::AeroSyscallError> {
        let handle = self
            .get_handle(fd)
            .ok_or(aero_syscall::AeroSyscallError::EINVAL)?;

        let mut files = self.0.write();

        if let Some((index, f)) = files.iter_mut().enumerate().find(|e| e.1.is_none()) {
            *f = Some(handle.duplicate(flags)?);
            Ok(index)
        } else {
            files.push(Some(handle.duplicate(flags)?));
            Ok(files.len() - 1)
        }
    }

    pub fn deep_clone(&self) -> Self {
        let files = self.0.read();
        Self(RwLock::new(files.clone()))
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

    /// Closes a file descriptor, so that its no longer referes to any file
    /// and can be resued. This function will return false if the provided file
    /// descriptor index was invalid.
    pub fn close_file(&self, fd: usize) -> bool {
        let mut files = self.0.write();

        if let Some(file) = files.get_mut(fd) {
            if let Some(handle) = file {
                handle.inode.inode().close(handle.flags);
                *file = None;

                return true;
            }
        }

        false
    }
}
