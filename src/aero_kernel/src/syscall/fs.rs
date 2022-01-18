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

use aero_syscall::{AeroSyscallError, OpenFlags};

use crate::fs::inode::DirEntry;
use crate::fs::pipe::Pipe;
use crate::fs::{self, lookup_path, LookupMode};
use crate::userland::scheduler;

use crate::fs::Path;
use crate::utils::{validate_slice, validate_slice_mut, validate_str};

pub fn write(fd: usize, buffer: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    if handle
        .flags
        .intersects(OpenFlags::O_WRONLY | OpenFlags::O_RDWR)
    {
        let buffer = validate_slice(buffer as *const u8, size).ok_or(AeroSyscallError::EINVAL)?;
        Ok(handle.write(buffer)?)
    } else {
        Err(AeroSyscallError::EACCES)
    }
}

pub fn read(fd: usize, buffer: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    if handle
        .flags
        .intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR)
    {
        let buffer = validate_slice_mut(buffer as *mut u8, size).ok_or(AeroSyscallError::EINVAL)?;
        Ok(handle.read(buffer)?)
    } else {
        Err(AeroSyscallError::EACCES)
    }
}

pub fn open(_fd: usize, path: usize, len: usize, mode: usize) -> Result<usize, AeroSyscallError> {
    let mut flags = OpenFlags::from_bits(mode).ok_or(AeroSyscallError::EINVAL)?;

    if !flags.intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR | OpenFlags::O_WRONLY) {
        flags.insert(OpenFlags::O_RDONLY);
    }

    let path = validate_str(path as *const u8, len).ok_or(AeroSyscallError::EINVAL)?;
    let path = Path::new(path);

    let mut lookup_mode = LookupMode::None;

    if flags.contains(OpenFlags::O_CREAT) {
        lookup_mode = LookupMode::Create;
    }

    let inode = fs::lookup_path_with_mode(path, lookup_mode)?;

    if flags.contains(OpenFlags::O_DIRECTORY) && !inode.inode().metadata()?.is_directory() {
        return Err(AeroSyscallError::ENOTDIR);
    }

    if flags.contains(OpenFlags::O_TRUNC) {
        inode.inode().truncate(0)?;
    }

    Ok(scheduler::get_scheduler()
        .current_task()
        .file_table
        .open_file(inode, flags)?)
}

pub fn dup(fd: usize, flags: usize) -> Result<usize, AeroSyscallError> {
    let task = scheduler::get_scheduler().current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)? & OpenFlags::O_CLOEXEC;

    task.file_table.duplicate(fd, flags)
}

pub fn getdents(fd: usize, buffer: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    let buffer = validate_slice_mut(buffer as *mut u8, size).ok_or(AeroSyscallError::EINVAL)?;
    Ok(handle.get_dents(buffer)?)
}

pub fn close(fd: usize) -> Result<usize, AeroSyscallError> {
    let res = scheduler::get_scheduler()
        .current_task()
        .file_table
        .close_file(fd);

    if res {
        Ok(0x00)
    } else {
        // FD isn't a valid open file descriptor.
        Err(AeroSyscallError::EBADFD)
    }
}

pub fn chdir(path: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let buffer = validate_str(path as *mut u8, size).ok_or(AeroSyscallError::EINVAL)?;
    let inode = fs::lookup_path(Path::new(buffer))?;

    if !inode.inode().metadata()?.is_directory() {
        // A component of path is not a directory.
        return Err(AeroSyscallError::ENOTDIR);
    }

    scheduler::get_scheduler().current_task().set_cwd(inode);
    Ok(0x00)
}

pub fn mkdirat(dfd: usize, path: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let path_str = validate_str(path as *mut u8, size).ok_or(AeroSyscallError::EINVAL)?;
    let path = Path::new(path_str);

    // NOTE: If the pathname given in pathname is relative, then it is interpreted
    // relative to the directory referred to by the file descriptor (rather than relative
    // to the current working directory of the calling task, as is done by mkdir() for a
    // relative pathname).
    let (parent_inode, child) = if path.is_absolute() {
        let (path, child) = path.parent_and_basename();
        (fs::lookup_path(path)?.inode(), child)
    } else {
        // If pathname is relative and fd is the special value AT_FDCWD, then
        // pathname is interpreted relative to the current working directory of the
        // calling task.
        if dfd as isize == aero_syscall::AT_FDCWD {
            let cwd = scheduler::get_scheduler().current_task().get_cwd_dirent();
            (cwd.inode(), path.as_str())
        } else {
            let handle = scheduler::get_scheduler()
                .current_task()
                .file_table
                .get_handle(dfd)
                .ok_or(AeroSyscallError::EBADFD)?;

            (handle.inode(), path.as_str())
        }
    };

    if !parent_inode.metadata()?.is_directory() {
        // A component of path is not a directory.
        return Err(AeroSyscallError::ENOTDIR);
    }

    if ["", ".", ".."].contains(&path_str) {
        // Cannot create a directory with a name of "", ".", or "..".
        return Err(AeroSyscallError::EEXIST);
    }

    parent_inode.mkdir(child)?;
    Ok(0x00)
}

#[inline]
pub fn mkdir(path: usize, size: usize) -> Result<usize, AeroSyscallError> {
    mkdirat(aero_syscall::AT_FDCWD as _, path, size)
}

pub fn rmdir(path: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let path_str = validate_str(path as *mut u8, size).ok_or(AeroSyscallError::EINVAL)?;
    let path = Path::new(path_str);

    let (_, child) = path.parent_and_basename();
    let inode = fs::lookup_path(path)?;

    if !inode.inode().metadata()?.is_directory() {
        // ENOTDIR: A component used as a directory in pathname, is not in fact,
        // a directory.
        return Err(AeroSyscallError::ENOTDIR);
    }

    inode.inode().rmdir(child)?;
    inode.drop_from_cache();
    Ok(0x00)
}

pub fn getcwd(buffer: usize, size: usize) -> Result<usize, AeroSyscallError> {
    // Invalid value of the size argument is zero and buffer is not a
    // null pointer.
    if size == 0x00 && buffer != 0x00 {
        return Err(AeroSyscallError::EINVAL);
    }

    let buffer = validate_slice_mut(buffer as *mut u8, size).ok_or(AeroSyscallError::EINVAL)?;
    let cwd = scheduler::get_scheduler().current_task().get_cwd();

    buffer[..cwd.len()].copy_from_slice(cwd.as_bytes());
    Ok(cwd.len())
}

pub fn ioctl(fd: usize, command: usize, argument: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    Ok(handle.inode().ioctl(command, argument)?)
}

pub fn seek(fd: usize, offset: usize, whence: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    Ok(handle.seek(offset as isize, aero_syscall::SeekWhence::from(whence))?)
}

pub fn pipe(fds: usize, flags: usize) -> Result<usize, AeroSyscallError> {
    let flags = OpenFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;
    let fds = validate_slice_mut(fds as *mut usize, 2).ok_or(AeroSyscallError::EINVAL)?;

    let pipe = Pipe::new();

    let entry = DirEntry::from_inode(pipe);

    let flags_1 = OpenFlags::O_RDONLY | (flags & OpenFlags::O_CLOEXEC);
    let flags_2 = OpenFlags::O_WRONLY | (flags & OpenFlags::O_CLOEXEC);

    let current_task = scheduler::get_scheduler().current_task();

    let fd1 = current_task.file_table.open_file(entry.clone(), flags_1)?;
    let fd2 = current_task.file_table.open_file(entry, flags_2);

    // If there was an error in opening the second file descriptor,
    // then close the first file descriptor. Just to be safe :^)
    let fd2 = match fd2 {
        Err(err) => {
            current_task.file_table.close_file(fd1);
            return Err(err.into());
        }

        Ok(fd2) => fd2,
    };

    fds[0] = fd1;
    fds[1] = fd2;

    Ok(0x00)
}

pub fn unlink(
    fd: usize,
    path: usize,
    path_size: usize,
    flags: usize,
) -> Result<usize, AeroSyscallError> {
    let path_str = validate_str(path as *mut u8, path_size).ok_or(AeroSyscallError::EINVAL)?;
    let path = Path::new(path_str);

    // TODO: Make use of the open flags.
    let _flags = OpenFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;
    let name = path.container();

    if fd as isize == aero_syscall::AT_FDCWD {
        let file = fs::lookup_path(path)?;

        if let Some(dir) = file.parent() {
            let metadata = file.inode().metadata()?;

            if metadata.is_file() {
                dir.inode().unlink(name.as_str())?;
                file.drop_from_cache();
            }
        }
    } else {
        unimplemented!()
    }

    Ok(0x00)
}

pub fn access(
    fd: usize,
    path: usize,
    path_size: usize,
    _mode: usize,
    _flags: usize,
) -> Result<usize, AeroSyscallError> {
    let path_str = validate_str(path as *mut u8, path_size).ok_or(AeroSyscallError::EINVAL)?;
    let path = Path::new(path_str);

    if fd as isize == aero_syscall::AT_FDCWD {
        lookup_path(path)?;
        Ok(0x00)
    } else {
        // TODO: Implement atfd access
        unimplemented!()
    }
}
