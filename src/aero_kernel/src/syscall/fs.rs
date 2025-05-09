// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use core::fmt;

use aero_syscall::prelude::*;
use aero_syscall::signal::SigProcMask;
use aero_syscall::{AtFlags, OpenFlags, Stat, TimeSpec, AT_FDCWD};
use alloc::sync::{Arc, Weak};

use crate::fs::cache::{self, DirCacheImpl};
use crate::fs::epoll::EPoll;
use crate::fs::eventfd::EventFd;
use crate::fs::file_table::{DuplicateHint, FileHandle};
use crate::fs::inode::{DirEntry, PollTable};
use crate::fs::pipe::Pipe;
use crate::fs::{self, LookupMode};
use crate::syscall::SysArg;
use crate::userland::scheduler;

use crate::fs::Path;

#[derive(Debug, Copy, Clone)]
pub struct FileDescriptor(usize);

impl FileDescriptor {
    /// Returns the file handle associated with this file descriptor.
    ///
    /// ## Errors
    /// * `EBADFD`: The file descriptor is not a valid open file descriptor.
    pub fn handle(&self) -> aero_syscall::Result<Arc<FileHandle>> {
        scheduler::current_thread()
            .file_table
            .get_handle(self.0)
            .ok_or(SyscallError::EBADFD)
    }
}

impl fmt::Display for FileDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(file_handle) = self.handle() {
            let path = file_handle.inode.absolute_path();
            write!(f, "{{ {} -> {} }}", self.0, path)
        } else {
            write!(f, "{{ {} -> INVALID }}", self.0)
        }
    }
}

impl super::SysArg for FileDescriptor {
    fn from_usize(value: usize) -> Self {
        Self(value)
    }
}

impl From<FileDescriptor> for usize {
    fn from(val: FileDescriptor) -> Self {
        val.0
    }
}

#[syscall]
pub fn write(fd: FileDescriptor, buffer: &[u8]) -> Result<usize, SyscallError> {
    // FIXME(heck for xeyes): fnctl should update the open flags!
    //
    // if handle
    //     .flags
    //     .intersects(OpenFlags::O_WRONLY | OpenFlags::O_RDWR)
    // {
    Ok(fd.handle()?.write(buffer)?)
    // } else {
    //     Err(SyscallError::EACCES)
    // }
}

#[syscall]
pub fn read(fd: FileDescriptor, buffer: &mut [u8]) -> Result<usize, SyscallError> {
    // if handle
    //     .flags
    //     .read()
    //     .intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR)
    // {
    Ok(fd.handle()?.read(buffer)?)
    // } else {
    //     Err(SyscallError::EACCES)
    // }
}

#[syscall]
pub fn open(fd: usize, path: &Path, flags: usize, _mode: usize) -> Result<usize, SyscallError> {
    let current_thread = scheduler::current_thread();
    let at = match fd as isize {
        AT_FDCWD if !path.is_absolute() => current_thread.cwd_dirent(),
        _ if !path.is_absolute() => {
            let ent = FileDescriptor::from_usize(fd).handle()?.inode.clone();
            assert!(ent.inode().metadata()?.is_directory());
            ent
        }
        _ => fs::root_dir().clone(),
    };

    let mut flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;

    if !flags.intersects(OpenFlags::O_RDONLY | OpenFlags::O_RDWR | OpenFlags::O_WRONLY) {
        flags.insert(OpenFlags::O_RDONLY);
    }

    let mut lookup_mode = LookupMode::None;

    if flags.contains(OpenFlags::O_CREAT) {
        lookup_mode = LookupMode::Create;
    }

    let inode = fs::lookup_path_with(at, path, lookup_mode, true)?;

    if flags.contains(OpenFlags::O_DIRECTORY) && !inode.inode().metadata()?.is_directory() {
        return Err(SyscallError::ENOTDIR);
    }

    if flags.contains(OpenFlags::O_TRUNC) {
        inode.inode().truncate(0)?;
    }

    Ok(current_thread.file_table.open_file(inode.clone(), flags)?)
}

#[syscall]
pub fn dup(fd: FileDescriptor, flags: usize) -> Result<usize, SyscallError> {
    let task = scheduler::get_scheduler().current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)? & OpenFlags::O_CLOEXEC;

    task.file_table
        .duplicate(fd.into(), DuplicateHint::Any, flags)
}

#[syscall]
pub fn dup2(fd: FileDescriptor, new_fd: usize, flags: usize) -> Result<usize, SyscallError> {
    let task = scheduler::get_scheduler().current_task();
    let flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)? & OpenFlags::O_CLOEXEC;

    task.file_table
        .duplicate(fd.into(), DuplicateHint::Exact(new_fd), flags)
}

#[syscall]
pub fn getdents(fd: FileDescriptor, buffer: &mut [u8]) -> Result<usize, SyscallError> {
    Ok(fd.handle()?.get_dents(buffer)?)
}

#[syscall]
pub fn close(fd: FileDescriptor) -> Result<usize, SyscallError> {
    let res = scheduler::current_thread().file_table.close_file(fd.into());

    if res {
        Ok(0)
    } else {
        // FD isn't a valid open file descriptor.
        Err(SyscallError::EBADFD)
    }
}

#[syscall]
pub fn chdir(fd: usize, path: &Path) -> Result<usize, SyscallError> {
    let current_thread = scheduler::current_thread();
    let at = match fd as isize {
        AT_FDCWD if !path.is_absolute() => current_thread.cwd_dirent(),
        _ if !path.is_absolute() => {
            let ent = FileDescriptor::from_usize(fd).handle()?.inode.clone();
            assert!(ent.inode().metadata()?.is_directory());
            ent
        }
        _ => fs::root_dir().clone(),
    };

    if path.is_empty() {
        current_thread.set_cwd(at);
        return Ok(0);
    }

    let ent = fs::lookup_path_with(at, path, LookupMode::None, true)?;
    if !ent.inode().metadata()?.is_directory() {
        return Err(SyscallError::ENOTDIR);
    }

    current_thread.set_cwd(ent);
    Ok(0)
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
            let cwd = scheduler::current_thread().cwd_dirent();
            (cwd.inode(), path.as_str())
        } else {
            let handle = scheduler::current_thread()
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
pub fn rmdir(path: &Path) -> Result<usize, SyscallError> {
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
    let cwd = scheduler::current_thread().get_cwd();
    log::debug!("getcwd: {}", cwd);

    // FIXME: fix this before commiting
    buffer.fill(0);
    buffer[..cwd.len()].copy_from_slice(cwd.as_bytes());

    // TOOD: mlibc doesnt give a shit and will increase the buf size till it fits. make it smarter.
    Ok(cwd.len())
}

#[syscall]
pub fn ioctl(fd: FileDescriptor, command: usize, argument: usize) -> Result<usize, SyscallError> {
    let handle = fd.handle()?;

    match command {
        // Sets the close-on-exec file descriptor flag. This is equivalent
        // to `fcntl(fd, F_SETFD, FD_CLOEXEC)`
        FIOCLEX => {
            let flags = handle.flags();
            handle.set_flags(flags | OpenFlags::O_CLOEXEC);
            Ok(0)
        }

        FIONBIO => {
            let flags = handle.flags();
            handle.set_flags(flags | OpenFlags::O_NONBLOCK);
            Ok(0)
        }

        // Handle file specific ioctl:
        _ => Ok(handle.inode().ioctl(command, argument)?),
    }
}

#[syscall]
pub fn seek(fd: FileDescriptor, offset: usize, whence: usize) -> Result<usize, SyscallError> {
    let handle = fd.handle()?;
    Ok(handle.seek(offset as isize, aero_syscall::SeekWhence::from(whence))?)
}

#[syscall]
pub fn pipe(fds: &mut [i32; 2], flags: usize) -> Result<usize, SyscallError> {
    let flags = OpenFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;
    let pipe = Pipe::new();

    let entry = DirEntry::from_inode(pipe, String::from("<pipe>"));

    let flags_1 = OpenFlags::O_RDONLY | flags;
    let flags_2 = OpenFlags::O_WRONLY | flags;

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
pub fn access(fd: usize, path: &Path, _mode: usize, flags: usize) -> Result<usize, SyscallError> {
    let at = match fd as isize {
        AT_FDCWD if !path.is_absolute() => scheduler::current_thread().cwd_dirent(),
        _ if !path.is_absolute() => FileDescriptor::from_usize(fd).handle()?.inode.clone(),
        _ => fs::root_dir().clone(),
    };

    let flags = AtFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;

    let resolve_last = !flags.contains(AtFlags::SYMLINK_NOFOLLOW);
    let _ = fs::lookup_path_with(at, path, LookupMode::None, resolve_last)?;

    Ok(0)
}

const SETFL_MASK: OpenFlags = OpenFlags::from_bits_truncate(
    OpenFlags::O_APPEND.bits()
        | OpenFlags::O_NONBLOCK.bits()
        // | OpenFlags::O_NDELAY.bits()
        | OpenFlags::O_DIRECT.bits()
        | OpenFlags::O_NOATIME.bits(),
);

#[syscall]
pub fn fcntl(fd: FileDescriptor, command: usize, arg: usize) -> Result<usize, SyscallError> {
    let handle = fd.handle()?;

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
        aero_syscall::prelude::F_DUPFD => scheduler::current_thread().file_table.duplicate(
            fd.into(),
            DuplicateHint::GreatorOrEqual(arg),
            handle.flags(),
        ),

        aero_syscall::prelude::F_DUPFD_CLOEXEC => scheduler::current_thread().file_table.duplicate(
            fd.into(),
            DuplicateHint::GreatorOrEqual(arg),
            handle.flags() | OpenFlags::O_CLOEXEC,
        ),

        // Get the value of file descriptor flags.
        aero_syscall::prelude::F_GETFD => {
            let flags = handle.flags();
            let mut result = FdFlags::empty();

            if flags.contains(OpenFlags::O_CLOEXEC) {
                result.insert(FdFlags::CLOEXEC);
            }

            Ok(result.bits())
        }

        // Set the value of file descriptor flags:
        aero_syscall::prelude::F_SETFD => {
            let mut flags = handle.flags();
            let fd_flags = FdFlags::from_bits_truncate(arg);

            if fd_flags.contains(FdFlags::CLOEXEC) {
                flags.insert(OpenFlags::O_CLOEXEC);
            } else {
                flags.remove(OpenFlags::O_CLOEXEC);
            }

            handle.set_flags(flags);
            Ok(0)
        }

        // Get the value of file status flags:
        aero_syscall::prelude::F_GETFL => Ok(handle.flags().bits()),

        aero_syscall::prelude::F_SETFL => {
            let flags = OpenFlags::from_bits_truncate(arg);
            let old_flags = handle.flags();
            handle.set_flags((flags & SETFL_MASK) | (old_flags & !SETFL_MASK));

            Ok(0)
        }

        aero_syscall::prelude::F_SETLKW | aero_syscall::prelude::F_SETLK => {
            log::warn!("fcntl: F_SETLKW,F_SETLK are a stub!");
            Ok(0)
        }

        _ => {
            log::error!("fcntl: unknown command {command}");
            Ok(0)
        }
    }
}

#[syscall]
pub fn fstat(fd: usize, path: &Path, flags: usize, stat: &mut Stat) -> Result<usize, SyscallError> {
    let at = match fd as isize {
        AT_FDCWD if !path.is_absolute() => scheduler::current_thread().cwd_dirent(),
        _ if !path.is_absolute() => FileDescriptor::from_usize(fd).handle()?.inode.clone(),
        _ => fs::root_dir().clone(),
    };

    // TODO: derive(SysArg) for bitflags.
    let flags = AtFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;
    assert!(!flags.intersects(AtFlags::EACCESS | AtFlags::REMOVEDIR));

    if path.is_empty() {
        if !flags.contains(AtFlags::EMPTY_PATH) {
            return Err(SyscallError::EINVAL);
        }

        *stat = at.inode().stat()?;
        return Ok(0);
    }

    let resolve_last = !flags.contains(AtFlags::SYMLINK_NOFOLLOW);
    let ent = fs::lookup_path_with(at, path, LookupMode::None, resolve_last)?;
    *stat = ent.inode().stat()?;
    Ok(0)
}

#[syscall]
pub fn stat(path: &Path, stat: &mut Stat) -> Result<usize, SyscallError> {
    let file = fs::lookup_path(path)?;
    *stat = file.inode().stat()?;
    Ok(0)
}

#[syscall]
pub fn read_link(path: &Path, buffer: &mut [u8]) -> Result<usize, SyscallError> {
    // XXX: lookup_path with automatically resolve the link.
    let cwd = if !path.is_absolute() {
        scheduler::current_thread().cwd_dirent()
    } else {
        fs::root_dir().clone()
    };

    let file = fs::lookup_path_with(cwd.clone(), path, LookupMode::None, false)?.inode();
    if !file.metadata()?.is_symlink() {
        return Err(SyscallError::EINVAL);
    }

    let resolved_path = file.resolve_link()?;
    let resolved_path = if resolved_path.is_absolute() {
        resolved_path
    } else {
        cwd.absolute_path().join(resolved_path)
    };

    let size = core::cmp::min(resolved_path.as_str().len(), buffer.len());

    log::warn!("Orig: {path:?} -> {resolved_path}");

    buffer[..size].copy_from_slice(&resolved_path.as_bytes()[..size]);
    Ok(size)
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
    epfd: FileDescriptor,
    mode: usize,
    fd: usize,
    event: &mut EPollEvent,
) -> Result<usize, SyscallError> {
    let epfd = epfd.handle()?;

    let epoll = epfd
        .inode()
        .downcast_arc::<EPoll>()
        .ok_or(SyscallError::EINVAL)?;

    match mode {
        EPOLL_CTL_ADD => {
            epoll.add_event(fd, *event)?;
            Ok(0)
        }

        EPOLL_CTL_DEL => {
            epoll.remove_event(fd)?;
            Ok(0)
        }

        EPOLL_CTL_MOD => {
            epoll.update_event(fd, *event)?;
            Ok(0)
        }

        _ => unreachable!("epoll_ctl: unknown mode {mode}"),
    }
}

#[syscall]
pub fn epoll_pwait(
    epfd: FileDescriptor,
    event: &mut [EPollEvent],
    timeout: usize,
    sigmask: usize,
) -> Result<usize, SyscallError> {
    let max_events = event.len();

    let current_task = scheduler::get_scheduler().current_task();
    let signals = current_task.signals();

    let epfd = epfd.handle()?;
    let epfd = epfd
        .inode()
        .downcast_arc::<EPoll>()
        .ok_or(SyscallError::EINVAL)?;

    let mut old_mask = 0;

    // Update the signal mask.
    signals.set_mask(SigProcMask::Set, Some(sigmask as u64), Some(&mut old_mask));

    let result = epfd.wait(event, max_events, timeout)?;

    // Restore the original signal mask.
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
    // SAFETY: The pointers to the file system are valid since we know that there are
    // strong references to it.
    //
    // TODO: Should this be moved to the inode impl?
    if !Weak::ptr_eq(
        &dest_dir.weak_filesystem().unwrap(),
        &src.inode().weak_filesystem().unwrap(),
    ) {
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

        // TODO: If an invalid file descriptor is provided then return EBADFD. Not implemented
        // currently, since the init process (libc?) tries to POLL on the stdout, stdin and
        // stdout file descriptors which are currently not present.
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
        scheduler::get_scheduler().await_io()?;

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
    if fds.is_empty() {
        return Ok(0);
    }

    // The timeout can be NULL.
    let timeout = if timeout != 0x00 {
        Some(crate::utils::validate_ptr(timeout as *const TimeSpec)?)
    } else {
        None
    };

    let current_task = scheduler::get_scheduler().current_task();
    let signals = current_task.signals();

    let mut old_mask = 0;

    // Update the signal mask.
    signals.set_mask(SigProcMask::Set, Some(sigmask as u64), Some(&mut old_mask));

    let n = do_poll(fds, timeout)?;

    // Restore the original signal mask.
    signals.set_mask(SigProcMask::Set, Some(old_mask), None);
    Ok(n)
}

#[syscall]
pub fn rename(src: &Path, dest: &Path) -> Result<usize, SyscallError> {
    let src = fs::lookup_path(src)?;
    let (dest, name) = {
        let (dir, name) = dest.parent_and_basename();
        (fs::lookup_path(dir)?, name)
    };

    dest.inode().rename(src.clone(), name)?;

    cache::dcache().rehash(src.clone(), || {
        src.set_name(name);
        src.set_parent(dest);
    });
    Ok(0)
}

#[syscall]
pub fn symlink(link_dirfd: usize, target: &Path, linkpath: &Path) -> Result<usize, SyscallError> {
    // TODO(andypython): the following code is reused in a couple of places. Isolate this inside the
    // syscall parsing for FileDescriptor with an argument as a generic specifing the allowance
    // of this value.
    //
    // If the pathname given in `linkpath` is relative, then it is interpreted relative to the
    // directory referred to by the file descriptor `link_dirfd`.
    let at = match link_dirfd as isize {
        AT_FDCWD if !linkpath.is_absolute() => scheduler::current_thread().cwd_dirent(),
        _ if !linkpath.is_absolute() => FileDescriptor::from_usize(link_dirfd)
            .handle()?
            .inode
            .clone(),
        _ => fs::root_dir().clone(),
    };

    let ent = fs::lookup_path_with(at, linkpath, LookupMode::Create, false)?;
    ent.inode().symlink(target)?;

    Ok(0)
}
