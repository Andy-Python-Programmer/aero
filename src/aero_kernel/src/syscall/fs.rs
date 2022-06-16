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

use aero_syscall::prelude::*;
use aero_syscall::signal::SigProcMask;
use aero_syscall::{AeroSyscallError, OpenFlags, Stat};

use crate::fs::epoll::EPoll;
use crate::fs::eventfd::EventFd;
use crate::fs::file_table::DuplicateHint;
use crate::fs::inode::{DirEntry, INodeInterface};
use crate::fs::pipe::Pipe;
use crate::fs::{self, lookup_path, LookupMode};
use crate::userland::scheduler;

use crate::fs::Path;
use crate::utils::downcast;

#[syscall]
pub fn write(fd: usize, buffer: &[u8]) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    if handle
        .flags
        .intersects(OpenFlags::O_WRONLY | OpenFlags::O_RDWR)
    {
        Ok(handle.write(buffer)?)
    } else {
        Err(AeroSyscallError::EACCES)
    }
}

#[syscall]
pub fn read(fd: usize, buffer: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    if handle
        .flags
        .intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR)
    {
        Ok(handle.read(buffer)?)
    } else {
        Err(AeroSyscallError::EACCES)
    }
}

#[syscall]
pub fn open(_fd: usize, path: &Path, mode: usize) -> Result<usize, AeroSyscallError> {
    let mut flags = OpenFlags::from_bits(mode).ok_or(AeroSyscallError::EINVAL)?;

    if !flags.intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR | OpenFlags::O_WRONLY) {
        flags.insert(OpenFlags::O_RDONLY);
    }

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

#[syscall]
pub fn dup(fd: usize, flags: usize) -> Result<usize, AeroSyscallError> {
    let task = scheduler::get_scheduler().current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)? & OpenFlags::O_CLOEXEC;

    task.file_table.duplicate(fd, DuplicateHint::Any, flags)
}

#[syscall]
pub fn dup2(fd: usize, new_fd: usize, flags: usize) -> Result<usize, AeroSyscallError> {
    let task = scheduler::get_scheduler().current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)? & OpenFlags::O_CLOEXEC;

    task.file_table
        .duplicate(fd, DuplicateHint::Exact(new_fd), flags)
}

#[syscall]
pub fn getdents(fd: usize, buffer: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    Ok(handle.get_dents(buffer)?)
}

#[syscall]
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

#[syscall]
pub fn chdir(path: &str) -> Result<usize, AeroSyscallError> {
    let inode = fs::lookup_path(Path::new(path))?;

    if !inode.inode().metadata()?.is_directory() {
        // A component of path is not a directory.
        return Err(AeroSyscallError::ENOTDIR);
    }

    scheduler::get_scheduler().current_task().set_cwd(inode);
    Ok(0x00)
}

#[syscall]
pub fn mkdirat(dfd: usize, path: &Path) -> Result<usize, AeroSyscallError> {
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

    if ["", ".", ".."].contains(&path.as_str()) {
        // Cannot create a directory with a name of "", ".", or "..".
        return Err(AeroSyscallError::EEXIST);
    }

    parent_inode.mkdir(child)?;
    Ok(0x00)
}

#[syscall]
pub fn rmdir(path: &str) -> Result<usize, AeroSyscallError> {
    let path = Path::new(path);

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

#[syscall]
pub fn getcwd(buffer: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let cwd = scheduler::get_scheduler().current_task().get_cwd();

    buffer[..cwd.len()].copy_from_slice(cwd.as_bytes());
    Ok(cwd.len())
}

#[syscall]
pub fn ioctl(fd: usize, command: usize, argument: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    match command {
        // Sets the close-on-exec file descriptor flag. This is equivalent
        // to `fcntl(fd, F_SETFD, FD_CLOEXEC)`
        FIOCLEX => {
            handle.fd_flags.lock().insert(FdFlags::CLOEXEC);
            return Ok(0x00);
        }

        // Handle file specific ioctl:
        _ => Ok(handle.inode().ioctl(command, argument)?),
    }
}

#[syscall]
pub fn seek(fd: usize, offset: usize, whence: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    Ok(handle.seek(offset as isize, aero_syscall::SeekWhence::from(whence))?)
}

#[syscall]
pub fn pipe(fds: &mut [usize; 2], flags: usize) -> Result<usize, AeroSyscallError> {
    let flags = OpenFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;
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

#[syscall]
pub fn unlink(fd: usize, path: &Path, flags: usize) -> Result<usize, AeroSyscallError> {
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

#[syscall]
pub fn access(
    fd: usize,
    path: &Path,
    _mode: usize,
    _flags: usize,
) -> Result<usize, AeroSyscallError> {
    if fd as isize == aero_syscall::AT_FDCWD {
        lookup_path(path)?;
        Ok(0x00)
    } else {
        // TODO: Implement atfd access
        unimplemented!()
    }
}

#[syscall]
pub fn fcntl(fd: usize, command: usize, arg: usize) -> Result<usize, AeroSyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    match command {
        // F_DUPFD_CLOEXEC and F_DUPFD:
        //
        // Duplicate the file descriptor `fd` using the lowest-numbered
        // available file descriptor greater than or equal to `arg`. This is
        // different from `dup2(2)`, which uses exactly the file descriptor
        // specified.
        //
        // F_DUPFD_CLOEXEC additionally sets the close-on-exec flag for the duplicate
        // file descriptor.
        aero_syscall::prelude::F_DUPFD_CLOEXEC => scheduler::get_scheduler()
            .current_task()
            .file_table
            .duplicate(fd, DuplicateHint::GreatorOrEqual(arg), OpenFlags::O_CLOEXEC),

        // Get the value of file descriptor flags.
        aero_syscall::prelude::F_GETFD => {
            let flags = handle.fd_flags.lock().bits();
            Ok(flags)
        }

        // Set the value of file descriptor flags:
        aero_syscall::prelude::F_SETFD => {
            let flags = FdFlags::from_bits(arg).ok_or(AeroSyscallError::EINVAL)?;
            handle.fd_flags.lock().insert(flags);

            Ok(0x00)
        }

        // Get the value of file status flags:
        aero_syscall::prelude::F_GETFL => {
            let flags = handle.flags.bits();
            Ok(flags)
        }

        _ => unimplemented!("fcntl: unknown command {command}"),
    }
}

#[syscall]
pub fn fstat(fd: usize, stat: &mut Stat) -> Result<usize, AeroSyscallError> {
    let file = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(AeroSyscallError::EBADFD)?;

    *stat = file.inode().stat()?;

    Ok(0)
}

#[syscall]
pub fn stat(path: &Path, stat: &mut Stat) -> Result<usize, AeroSyscallError> {
    let file = fs::lookup_path(path)?;

    *stat = file.inode().stat()?;

    Ok(0)
}

#[syscall]
pub fn read_link(path: &Path, _buffer: &mut [u8]) -> Result<usize, AeroSyscallError> {
    log::warn!("read_link: is a stub! (path={path:?})");

    Err(AeroSyscallError::EINVAL)
}

/// Returns a file descriptor referring to the new epoll instance.
#[syscall]
pub fn epoll_create(flags: usize) -> Result<usize, AeroSyscallError> {
    let _flags = EPollFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;

    let epoll_file = EPoll::new();
    let entry = DirEntry::from_inode(epoll_file);

    Ok(scheduler::get_scheduler()
        .current_task()
        .file_table
        .open_file(entry, OpenFlags::O_RDWR)?)
}

/// Used to add, modify, or remove entries in the interest list of the
/// epoll instance referred to by the file descriptor. It requests that
/// the operation be performed for the target file descriptor.
#[syscall]
pub fn epoll_ctl(
    epfd: usize,
    mode: usize,
    fd: usize,
    event: &mut EPollEvent,
) -> Result<usize, AeroSyscallError> {
    let epfd = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(epfd)
        .ok_or(AeroSyscallError::EBADFD)?;

    match mode {
        EPOLL_CTL_ADD => {
            let epoll = downcast::<dyn INodeInterface, EPoll>(&epfd.inode())
                .ok_or(AeroSyscallError::EINVAL)?;

            epoll.add_event(fd, event.clone())?;
            Ok(0)
        }

        _ => unreachable!("epoll_ctl: unknown mode {mode}"),
    }
}

#[syscall]
pub fn epoll_pwait(
    epfd: usize,
    event: &mut [&mut EPollEvent],
    timeout: usize,
    sigmask: usize,
) -> Result<usize, AeroSyscallError> {
    let max_events = event.len();

    let current_task = scheduler::get_scheduler().current_task();
    let signals = current_task.signals();

    let epfd = current_task
        .file_table
        .get_handle(epfd)
        .ok_or(AeroSyscallError::EBADFD)?;

    let epfd =
        downcast::<dyn INodeInterface, EPoll>(&epfd.inode()).ok_or(AeroSyscallError::EINVAL)?;

    let mut old_mask = 0;

    // Update the signal mask.
    signals.set_mask(SigProcMask::Set, Some(sigmask as u64), Some(&mut old_mask));

    let result = epfd.wait(event, max_events, timeout)?;

    // Restore the orignal signal mask.
    signals.set_mask(SigProcMask::Set, Some(old_mask), None);
    Ok(result)
}

#[syscall]
pub fn event_fd(_initval: usize, flags: usize) -> Result<usize, AeroSyscallError> {
    let flags = EventFdFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;
    assert!(!flags.contains(EventFdFlags::SEMAPHORE)); // todo: implement event fd semaphore support.

    let eventfd_file = EventFd::new();
    let entry = DirEntry::from_inode(eventfd_file);

    let current_task = scheduler::get_scheduler().current_task();

    Ok(current_task
        .file_table
        .open_file(entry, OpenFlags::O_RDWR)?)
}
