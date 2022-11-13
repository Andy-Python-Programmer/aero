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

use aero_syscall::signal::SigProcMask;
use aero_syscall::{prelude::*, TimeSpec};
use aero_syscall::{OpenFlags, Stat, SyscallError};

use crate::fs::cache::DirCacheImpl;
use crate::fs::epoll::EPoll;
use crate::fs::eventfd::EventFd;
use crate::fs::file_table::DuplicateHint;
use crate::fs::inode::{DirEntry, PollTable};
use crate::fs::pipe::Pipe;
use crate::fs::{self, lookup_path, LookupMode};
use crate::userland::scheduler;

use crate::fs::Path;

#[syscall]
pub fn write(fd: usize, buffer: &[u8]) -> Result<usize, SyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EBADFD)?;

    // FIXME(heck for xeyes): fnctl should update the open flags!
    //
    // if handle
    //     .flags
    //     .intersects(OpenFlags::O_WRONLY | OpenFlags::O_RDWR)
    // {
    Ok(handle.write(buffer)?)
    // } else {
    //     Err(SyscallError::EACCES)
    // }
}

#[syscall]
pub fn read(fd: usize, buffer: &mut [u8]) -> Result<usize, SyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EBADFD)?;

    if handle
        .flags
        .read()
        .intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR)
    {
        Ok(handle.read(buffer)?)
    } else {
        Err(SyscallError::EACCES)
    }
}

#[syscall]
pub fn open(_fd: usize, path: &Path, mode: usize) -> Result<usize, SyscallError> {
    let mut flags = OpenFlags::from_bits(mode).ok_or(SyscallError::EINVAL)?;

    if !flags.intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR | OpenFlags::O_WRONLY) {
        flags.insert(OpenFlags::O_RDONLY);
    }

    let mut lookup_mode = LookupMode::None;

    if flags.contains(OpenFlags::O_CREAT) {
        lookup_mode = LookupMode::Create;
    }

    let inode = fs::lookup_path_with_mode(path, lookup_mode)?;

    if flags.contains(OpenFlags::O_DIRECTORY) && !inode.inode().metadata()?.is_directory() {
        return Err(SyscallError::ENOTDIR);
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
pub fn dup(fd: usize, flags: usize) -> Result<usize, SyscallError> {
    let task = scheduler::get_scheduler().current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)? & OpenFlags::O_CLOEXEC;

    task.file_table.duplicate(fd, DuplicateHint::Any, flags)
}

#[syscall]
pub fn dup2(fd: usize, new_fd: usize, flags: usize) -> Result<usize, SyscallError> {
    let task = scheduler::get_scheduler().current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)? & OpenFlags::O_CLOEXEC;

    task.file_table
        .duplicate(fd, DuplicateHint::Exact(new_fd), flags)
}

#[syscall]
pub fn getdents(fd: usize, buffer: &mut [u8]) -> Result<usize, SyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EBADFD)?;

    Ok(handle.get_dents(buffer)?)
}

#[syscall]
pub fn close(fd: usize) -> Result<usize, SyscallError> {
    let res = scheduler::get_scheduler()
        .current_task()
        .file_table
        .close_file(fd);

    if res {
        Ok(0x00)
    } else {
        // FD isn't a valid open file descriptor.
        Err(SyscallError::EBADFD)
    }
}

#[syscall]
pub fn chdir(path: &str) -> Result<usize, SyscallError> {
    let inode = fs::lookup_path(Path::new(path))?;

    if !inode.inode().metadata()?.is_directory() {
        // A component of path is not a directory.
        return Err(SyscallError::ENOTDIR);
    }

    scheduler::get_scheduler().current_task().set_cwd(inode);
    Ok(0x00)
}

#[syscall]
pub fn mkdirat(dfd: usize, path: &Path) -> Result<usize, SyscallError> {
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
            let cwd = scheduler::get_scheduler().current_task().cwd_dirent();
            (cwd.inode(), path.as_str())
        } else {
            let handle = scheduler::get_scheduler()
                .current_task()
                .file_table
                .get_handle(dfd)
                .ok_or(SyscallError::EBADFD)?;

            (handle.inode(), path.as_str())
        }
    };

    if !parent_inode.metadata()?.is_directory() {
        // A component of path is not a directory.
        return Err(SyscallError::ENOTDIR);
    }

    if ["", ".", ".."].contains(&path.as_str()) {
        // Cannot create a directory with a name of "", ".", or "..".
        return Err(SyscallError::EEXIST);
    }

    parent_inode.mkdir(child)?;
    Ok(0x00)
}

#[syscall]
pub fn rmdir(path: &str) -> Result<usize, SyscallError> {
    let path = Path::new(path);

    let (_, child) = path.parent_and_basename();
    let inode = fs::lookup_path(path)?;

    if !inode.inode().metadata()?.is_directory() {
        // ENOTDIR: A component used as a directory in pathname, is not in fact,
        // a directory.
        return Err(SyscallError::ENOTDIR);
    }

    inode.inode().rmdir(child)?;
    inode.drop_from_cache();
    Ok(0x00)
}

#[syscall]
pub fn getcwd(buffer: &mut [u8]) -> Result<usize, SyscallError> {
    let cwd = scheduler::get_scheduler().current_task().get_cwd();

    buffer[..cwd.len()].copy_from_slice(cwd.as_bytes());
    Ok(cwd.len())
}

#[syscall]
pub fn ioctl(fd: usize, command: usize, argument: usize) -> Result<usize, SyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EBADFD)?;

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
pub fn seek(fd: usize, offset: usize, whence: usize) -> Result<usize, SyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EBADFD)?;

    Ok(handle.seek(offset as isize, aero_syscall::SeekWhence::from(whence))?)
}

#[syscall]
pub fn pipe(fds: &mut [i32; 2], flags: usize) -> Result<usize, SyscallError> {
    let flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;
    let pipe = Pipe::new();

    let entry = DirEntry::from_inode(pipe, String::from("<pipe>"));

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

    fds[0] = fd1 as i32;
    fds[1] = fd2 as i32;

    Ok(0x00)
}

#[syscall]
pub fn unlink(_fd: usize, _path: &Path, _flags: usize) -> Result<usize, SyscallError> {
    // let _flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;
    // let name = path.container();

    // if fd as isize == aero_syscall::AT_FDCWD {
    //     let file = fs::lookup_path(path)?;

    //     if let Some(dir) = file.parent() {
    //         let metadata = file.inode().metadata()?;

    //         if metadata.is_file() {
    //             dir.inode().unlink(name.as_str())?;
    //             file.drop_from_cache();
    //         }
    //     }
    // } else {
    //     unimplemented!()
    // }

    Ok(0x00)
}

#[syscall]
pub fn access(fd: usize, path: &Path, _mode: usize, _flags: usize) -> Result<usize, SyscallError> {
    if fd as isize == aero_syscall::AT_FDCWD {
        lookup_path(path)?;
        Ok(0x00)
    } else {
        // TODO: Implement atfd access
        unimplemented!()
    }
}

#[syscall]
pub fn fcntl(fd: usize, command: usize, arg: usize) -> Result<usize, SyscallError> {
    let handle = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EBADFD)?;

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
        aero_syscall::prelude::F_DUPFD => scheduler::get_scheduler()
            .current_task()
            .file_table
            .duplicate(fd, DuplicateHint::GreatorOrEqual(arg), *handle.flags.read()),

        aero_syscall::prelude::F_DUPFD_CLOEXEC => scheduler::get_scheduler()
            .current_task()
            .file_table
            .duplicate(
                fd,
                DuplicateHint::GreatorOrEqual(arg),
                *handle.flags.read() | OpenFlags::O_CLOEXEC,
            ),

        // Get the value of file descriptor flags.
        aero_syscall::prelude::F_GETFD => {
            let flags = handle.fd_flags.lock().bits();
            Ok(flags)
        }

        // Set the value of file descriptor flags:
        aero_syscall::prelude::F_SETFD => {
            let flags = FdFlags::from_bits(arg).ok_or(SyscallError::EINVAL)?;
            handle.fd_flags.lock().insert(flags);

            Ok(0)
        }

        // Get the value of file status flags:
        aero_syscall::prelude::F_GETFL => {
            let flags = handle.flags.read().bits();
            Ok(flags)
        }

        aero_syscall::prelude::F_SETFL => {
            let flags = OpenFlags::from_bits_truncate(arg);
            handle.flags.write().insert(flags);

            Ok(0)
        }

        _ => unimplemented!("fcntl: unknown command {command}"),
    }
}

#[syscall]
pub fn fstat(fd: usize, stat: &mut Stat) -> Result<usize, SyscallError> {
    let file = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(fd)
        .ok_or(SyscallError::EBADFD)?;

    *stat = file.inode().stat()?;

    Ok(0)
}

#[syscall]
pub fn stat(path: &Path, stat: &mut Stat) -> Result<usize, SyscallError> {
    let file = fs::lookup_path(path)?;

    *stat = file.inode().stat()?;

    Ok(0)
}

#[syscall]
pub fn read_link(path: &Path, _buffer: &mut [u8]) -> Result<usize, SyscallError> {
    log::warn!("read_link: is a stub! (path={path:?})");

    Err(SyscallError::EINVAL)
}

/// Returns a file descriptor referring to the new epoll instance.
#[syscall]
pub fn epoll_create(flags: usize) -> Result<usize, SyscallError> {
    let _flags = EPollFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;

    let epoll_file = EPoll::new();
    let entry = DirEntry::from_inode(epoll_file, String::from("<epoll>"));

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
) -> Result<usize, SyscallError> {
    let epfd = scheduler::get_scheduler()
        .current_task()
        .file_table
        .get_handle(epfd)
        .ok_or(SyscallError::EBADFD)?;

    let epoll = epfd
        .inode()
        .downcast_arc::<EPoll>()
        .ok_or(SyscallError::EINVAL)?;

    match mode {
        EPOLL_CTL_ADD => {
            epoll.add_event(fd, event.clone())?;
            Ok(0)
        }

        EPOLL_CTL_DEL => {
            epoll.remove_event(fd)?;
            Ok(0)
        }

        EPOLL_CTL_MOD => {
            epoll.update_event(fd, event.clone())?;
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
) -> Result<usize, SyscallError> {
    let max_events = event.len();

    let current_task = scheduler::get_scheduler().current_task();
    let signals = current_task.signals();

    let epfd = current_task
        .file_table
        .get_handle(epfd)
        .ok_or(SyscallError::EBADFD)?;

    let epfd = epfd
        .inode()
        .downcast_arc::<EPoll>()
        .ok_or(SyscallError::EINVAL)?;

    let mut old_mask = 0;

    // Update the signal mask.
    signals.set_mask(SigProcMask::Set, Some(sigmask as u64), Some(&mut old_mask));

    let result = epfd.wait(event, max_events, timeout)?;

    // Restore the orignal signal mask.
    signals.set_mask(SigProcMask::Set, Some(old_mask), None);
    Ok(result)
}

#[syscall]
pub fn event_fd(_initval: usize, flags: usize) -> Result<usize, SyscallError> {
    let flags = EventFdFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;
    assert!(!flags.contains(EventFdFlags::SEMAPHORE)); // todo: implement event fd semaphore support.

    let eventfd_file = EventFd::new();
    let entry = DirEntry::from_inode(eventfd_file, String::from("<eventfd>"));

    let current_task = scheduler::get_scheduler().current_task();

    Ok(current_task
        .file_table
        .open_file(entry, OpenFlags::O_RDWR)?)
}

/// Creates a new link (also known as a hard link) to an existing
/// file.
#[syscall]
pub fn link(src_path: &Path, dest_path: &Path) -> Result<usize, SyscallError> {
    let src = fs::lookup_path(src_path)?;
    let (dest_dir, dest_name) = dest_path.parent_and_basename();

    let dest_dir = fs::lookup_path(dest_dir)?.inode();

    // Cannot create a hardlink to a file on a different filesystem.
    //
    // SAFTEY: The pointers to the file system are valid since we know that there are
    // strong references to it.
    //
    // TODO: Should this be moved to the inode impl?
    if dest_dir.weak_filesystem().unwrap().as_ptr()
        != src.inode().weak_filesystem().unwrap().as_ptr()
    {
        return Err(SyscallError::EINVAL);
    }

    dest_dir.link(dest_name, src)?;
    Ok(0)
}

fn do_poll(fds: &mut [PollFd], timeout: Option<&TimeSpec>) -> Result<usize, SyscallError> {
    let current_task = scheduler::get_scheduler().current_task();

    let mut poll_table = PollTable::default();
    let mut n = 0;
    let mut refds = alloc::vec![];

    // Iterate over all the registered events and check if they are ready.
    for (i, fd) in fds.iter_mut().enumerate() {
        fd.revents = PollEventFlags::empty();

        // TODO: If an invalid file descriptor is provided then return EBADFD. Not implemented currently,
        // since the init process (libc?) tries to POLL on the stdout, stdin and stdout file descriptors
        // which are currently not present.
        //
        // One possible solution is to open the file descriptors when the init process
        // is a kernel process?
        let handle = match current_task.file_table.get_handle(fd.fd as usize) {
            Some(v) => v,
            None => {
                return Ok(0);
            }
        };

        let ready: PollEventFlags = handle.inode().poll(None)?.into();

        if !(ready & fd.events).is_empty() {
            // The registered event is ready; increment the number of ready events
            // and update revents mask for this event.
            fd.revents = ready & fd.events;
            n += 1;
            continue;
        }

        // Not ready; add the event to the poll table.
        handle.inode().poll(Some(&mut poll_table))?;
        refds.push((handle, i));
    }

    // If all events are ready, we can return now.
    if n > 0 {
        return Ok(n);
    }

    // Start the timer if timeout specified, if not, we can block indefinitely.
    if let Some(timeout) = timeout {
        // If the timeout is zero, then we have to return without blocking.
        if timeout.tv_nsec == 0 && timeout.tv_sec == 0 {
            return Ok(0);
        }
    }

    'search: loop {
        for (handle, index) in refds.iter() {
            let pollfd = &mut fds[*index];
            let ready: PollEventFlags = handle.inode().poll(None)?.into();

            if !(ready & pollfd.events).is_empty() {
                pollfd.revents = ready & pollfd.events;
                break 'search Ok(1);
            }
        }
    }
}

#[syscall]
pub fn poll(fds: &mut [PollFd], timeout: usize, sigmask: usize) -> Result<usize, SyscallError> {
    // Nothing to poll on.
    if fds.len() == 0 {
        return Ok(0);
    }

    // The timeout can be NULL.
    let timeout = if timeout != 0x00 {
        Some(crate::utils::validate_ptr(timeout as *const TimeSpec).ok_or(SyscallError::EINVAL)?)
    } else {
        None
    };

    let current_task = scheduler::get_scheduler().current_task();
    let signals = current_task.signals();

    let mut old_mask = 0;

    // Update the signal mask.
    signals.set_mask(SigProcMask::Set, Some(sigmask as u64), Some(&mut old_mask));

    let n = do_poll(fds, timeout)?;

    // Restore the orignal signal mask.
    signals.set_mask(SigProcMask::Set, Some(old_mask), None);
    Ok(n)
}
