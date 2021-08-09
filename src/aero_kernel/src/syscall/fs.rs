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

use aero_syscall::{AeroSyscallError, OpenFlags};

use crate::fs;
use crate::userland::scheduler;

use crate::fs::Path;
use crate::utils::{validate_slice, validate_str};

pub fn write(fd: usize, buffer: usize, len: usize) -> Result<usize, AeroSyscallError> {
    let fd = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd);

    if let Some(handle) = fd {
        if handle
            .flags
            .intersects(OpenFlags::O_WRONLY | OpenFlags::O_RDWR)
        {
            if let Some(buffer) = validate_slice(buffer as *const u8, len) {
                Ok(handle.write(buffer)?)
            } else {
                Err(AeroSyscallError::EINVAL)
            }
        } else {
            Err(AeroSyscallError::EACCES)
        }
    } else {
        Err(AeroSyscallError::EBADFD)
    }
}

pub fn open(_fd: usize, path: usize, len: usize, mode: usize) -> Result<usize, AeroSyscallError> {
    let mut flags = OpenFlags::from_bits(mode).ok_or(AeroSyscallError::EINVAL)?;

    if !flags.intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR | OpenFlags::O_WRONLY) {
        flags.insert(OpenFlags::O_RDONLY);
    }

    if let Some(path) = validate_str(path as *const u8, len) {
        let path = Path::new(path);
        let inode = fs::lookup_path(path)?;

        if flags.contains(OpenFlags::O_DIRECTORY) && inode.inode().metadata().is_directory() {
            return Err(AeroSyscallError::ENOTDIR);
        }

        if flags.contains(OpenFlags::O_TRUNC) {
            // FIXME(Andy-Python-Programmer): Implement file truncation.
            unimplemented!()
        }

        scheduler::get_scheduler()
            .current_task()
            .file_table
            .open_file(inode, flags)?;

        Ok(0)
    } else {
        Err(AeroSyscallError::EINVAL)
    }
}
