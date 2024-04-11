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

use aero_syscall::signal::{SigAction, SigProcMask};
use aero_syscall::*;
use spin::{Mutex, Once};

use crate::acpi::aml;
use crate::fs;
use crate::fs::Path;

use crate::mem::paging::VirtAddr;
use crate::userland::scheduler::{self, ExitStatus};
use crate::userland::signals::SignalEntry;
use crate::userland::task::sessions::SESSIONS;
use crate::userland::task::TaskId;
use crate::utils::sync::IrqGuard;

static HOSTNAME: Once<Mutex<String>> = Once::new();

fn hostname() -> &'static Mutex<String> {
    HOSTNAME.call_once(|| Mutex::new(String::from("aero")))
}

#[syscall(no_return)]
pub fn exit(status: usize) -> Result<usize> {
    #[cfg(all(test, feature = "ci"))]
    crate::emu::exit_qemu(crate::emu::ExitStatus::Success);

    #[cfg(not(feature = "ci"))]
    {
        let current_task = scheduler::get_scheduler().current_task();
        let pid = current_task.pid().as_usize();
        let path = current_task.path();

        log::trace!("exiting the process (pid={pid}, path={path:?}) with status: {status}");

        crate::unwind::unwind_stack_trace();
        scheduler::get_scheduler().exit(ExitStatus::Normal(status as isize));
    }
}

#[syscall]
pub fn uname(buffer: &mut Utsname) -> Result<usize> {
    fn init_array(fixed: &mut [u8; 65], init: &'static str) {
        let init_bytes = init.as_bytes();
        let len = init.len();

        fixed[..len].copy_from_slice(init_bytes);
        fixed[len..].fill(0);
    }

    init_array(&mut buffer.sysname, "Aero");
    init_array(&mut buffer.nodename, "unknown");
    init_array(&mut buffer.version, env!("CARGO_PKG_VERSION"));
    init_array(
        &mut buffer.release,
        concat!(env!("CARGO_PKG_VERSION"), "-aero"),
    );

    #[cfg(target_arch = "x86_64")]
    init_array(&mut buffer.machine, "x86_64");

    #[cfg(not(target_arch = "x86_64"))]
    init_array(&mut buffer.machine, "unknown");

    Ok(0x00)
}

#[syscall]
pub fn fork() -> Result<usize> {
    let scheduler = scheduler::get_scheduler();
    let forked = scheduler.current_task().fork();

    scheduler.register_task(forked.clone());
    Ok(forked.pid().as_usize())
}

#[syscall]
pub fn clone(entry: usize, stack: usize) -> Result<usize> {
    let scheduler = scheduler::get_scheduler();
    let cloned = scheduler.current_task().clone_process(entry, stack);

    scheduler.register_task(cloned.clone());
    Ok(cloned.pid().as_usize())
}

#[syscall]
pub fn kill(pid: usize, signal: usize) -> Result<usize> {
    // If pid is positive, then signal is sent to the process with that pid.
    if pid > 0 {
        crate::unwind::unwind_stack_trace();

        let task = scheduler::get_scheduler()
            .find_task(TaskId::new(pid))
            .ok_or(SyscallError::ESRCH)?;

        task.signal(signal);
        Ok(0)
    } else {
        unimplemented!()
    }
}

#[syscall(no_return)]
pub fn exec(path: &Path, args: usize, argc: usize, envs: usize, envc: usize) -> Result<usize> {
    let executable = fs::lookup_path(path)?;

    if executable.inode().metadata()?.is_directory() {
        return Err(SyscallError::EISDIR);
    }

    // NOTE: Neither args nor envs should be used after this point, the kernel
    // now has owned copies in args and environment variables.
    let argv = if argc > 0 {
        Some(super::exec_args_from_slice(args, argc))
    } else {
        None
    };
    let envv = if envc > 0 {
        Some(super::exec_args_from_slice(envs, envc))
    } else {
        None
    };

    scheduler::get_scheduler()
        .current_task()
        .exec(&executable, argv, envv)
        .expect("task: failed to exec task");

    unreachable!()
}

#[syscall]
pub fn log(msg: &str) -> Result<usize> {
    log::debug!("{}", msg);

    Ok(0x00)
}

#[syscall]
pub fn waitpid(pid: usize, status: &mut u32, flags: usize) -> Result<usize> {
    let flags = WaitPidFlags::from_bits_truncate(flags);
    let current_task = scheduler::get_scheduler().current_task();

    Ok(current_task.waitpid(pid as isize, status, flags)?)
}

#[syscall]
pub fn mmap(
    address: usize,
    size: usize,
    protection: usize,
    flags: usize,
    fd: usize,
    offset: usize,
) -> Result<usize> {
    let address = VirtAddr::new(address as u64);
    let protection = MMapProt::from_bits(protection).ok_or(SyscallError::EINVAL)?;
    let flags = MMapFlags::from_bits(flags).ok_or(SyscallError::EINVAL)?;

    let mut file = None;

    if fd as isize != -1 {
        file = Some(
            scheduler::get_scheduler()
                .current_task()
                .file_table
                .get_handle(fd)
                .ok_or(SyscallError::EBADF)?
                .dirnode(),
        );
    }

    if let Some(alloc) = scheduler::get_scheduler()
        .current_task()
        .vm()
        .mmap(address, size, protection, flags, offset, file)
    {
        Ok(alloc.as_u64() as usize)
    } else {
        Err(SyscallError::EFAULT)
    }
}

#[syscall]
pub fn munmap(address: usize, size: usize) -> Result<usize> {
    let address = VirtAddr::new(address as u64);

    if scheduler::get_scheduler()
        .current_task()
        .vm
        .munmap(address, size)
    {
        Ok(0x00)
    } else {
        Err(SyscallError::EFAULT)
    }
}

#[syscall]
pub fn mprotect(ptr: usize, size: usize, prot: usize) -> Result<usize> {
    let ptr = VirtAddr::new(ptr as _);
    let prot = MMapProt::from_bits(prot).ok_or(SyscallError::EINVAL)?;

    let task = scheduler::get_scheduler().current_task();
    task.vm().mprotect(ptr, size, prot);

    Ok(0)
}

#[syscall]
pub fn backtrace() -> Result<usize> {
    crate::unwind::unwind_stack_trace();
    Ok(0)
}

/// Enables syscall tracer for this process/thread.
///
/// When the tracer is enabled for a process, it also applies to any child processes spawned by the
/// process.
#[syscall]
pub fn trace() -> Result<usize> {
    scheduler::get_scheduler().current_task().enable_systrace();
    Ok(0)
}

#[syscall]
pub fn getpid() -> Result<usize> {
    Ok(scheduler::get_scheduler().current_task().pid().as_usize())
}

#[syscall]
pub fn getppid() -> Result<usize> {
    Ok(scheduler::get_scheduler()
        .current_task()
        .parent_pid()
        .as_usize())
}

#[syscall]
pub fn gettid() -> Result<usize> {
    Ok(scheduler::get_scheduler().current_task().tid().as_usize())
}

#[syscall]
pub fn gethostname(buffer: &mut [u8]) -> Result<usize> {
    let hostname = hostname().lock();
    let bytes = hostname.as_bytes();

    if bytes.len() > buffer.len() {
        Err(SyscallError::ENAMETOOLONG)
    } else {
        buffer[0..bytes.len()].copy_from_slice(bytes);
        buffer[bytes.len()] = b'\0';

        Ok(bytes.len())
    }
}

#[syscall]
pub fn info(struc: &mut SysInfo) -> Result<usize> {
    struc.uptime = crate::arch::time::get_uptime_ticks() as i64;

    Ok(0x00)
}

#[syscall]
pub fn sethostname(name: &[u8]) -> Result<usize> {
    match core::str::from_utf8(name) {
        Ok(name) => {
            *hostname().lock() = name.into();

            Ok(0)
        }
        Err(_) => Err(SyscallError::EINVAL),
    }
}

#[syscall]
pub fn sigprocmask(how: usize, set: *const u64, old_set: *mut u64) -> Result<usize> {
    let set = if set.is_null() {
        None
    } else {
        Some(unsafe { *set })
    };

    let old_set = if old_set.is_null() {
        None
    } else {
        Some(unsafe { &mut *old_set })
    };

    let how = SigProcMask::from(how as u64);

    scheduler::get_scheduler()
        .current_task()
        .signals()
        .set_mask(how, set, old_set);

    Ok(0)
}

#[syscall]
pub fn sigaction(
    sig: usize,
    sigact: *mut SigAction,
    sigreturn: usize,
    old: *mut SigAction,
) -> Result<usize> {
    let new = if sigact.is_null() {
        None
    } else {
        Some(unsafe { &mut *sigact })
    };

    let entry = if let Some(new) = new {
        Some(SignalEntry::from_sigaction(&*new, sigreturn)?)
    } else {
        None
    };

    let old = if old.is_null() {
        None
    } else {
        Some(unsafe { &mut *old })
    };

    let scheduler = scheduler::get_scheduler();
    let task = scheduler.current_task();
    let signals = task.signals();

    signals.set_signal(sig, entry, old);

    Ok(0)
}

#[syscall(no_return)]
pub fn shutdown() -> Result<usize> {
    fs::cache::dcache().log();

    fs::cache::clear_inode_cache();
    fs::cache::clear_dir_cache();

    let _guard = IrqGuard::new();
    aml::get_subsystem().enter_state(aml::SleepState::S5);

    unreachable!("aml: failed to shutdown (enter state S5)")
}

#[syscall]
pub fn getpgid(pid: usize) -> Result<usize> {
    let current_task = scheduler::current_thread();

    // If `pid` is 0, the process ID of the calling process is used.
    let task = if pid == 0 || pid == current_task.pid().as_usize() {
        current_task
    } else {
        scheduler::get_scheduler()
            .find_task(TaskId::new(pid))
            .ok_or(SyscallError::ESRCH)?
    };

    let group = SESSIONS.find_group(&task).unwrap();
    Ok(group.id())
}

#[syscall]
pub fn setpgid(pid: usize, pgid: usize) -> Result<usize> {
    let current_task = scheduler::current_thread();
    let task = if pid == 0 || pid == current_task.pid().as_usize() {
        current_task.clone()
    } else {
        let task = scheduler::get_scheduler()
            .find_task(TaskId::new(pid))
            .ok_or(SyscallError::ESRCH)?;

        if let Some(parent) = task.get_parent() {
            if parent.tid() != current_task.tid() {
                return Err(SyscallError::EPERM);
            }
        } else {
            return Err(SyscallError::EPERM);
        }

        task
    };

    if task.is_session_leader()
        || !task.is_process_leader()
        || task.session_id() != current_task.session_id()
    {
        return Err(SyscallError::EPERM);
    }

    log::error!("setpgid: is a stub! (pid={pid} pgid={pgid})");
    Ok(0)
}

#[syscall]
pub fn setsid() -> Result<usize> {
    let current_task = scheduler::get_scheduler().current_task();
    if current_task.is_group_leader() {
        return Err(SyscallError::EPERM);
    }

    SESSIONS.isolate(&current_task);
    Ok(0)
}
