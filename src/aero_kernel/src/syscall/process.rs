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

use aero_syscall::signal::{SigAction, SigProcMask};
use aero_syscall::{AeroSyscallError, MMapFlags, MMapProt, SysInfo, Utsname};
use alloc::string::String;
use spin::{Mutex, Once};

use crate::fs;
use crate::fs::Path;

use crate::mem::paging::VirtAddr;
use crate::userland::scheduler;
use crate::userland::signals::SignalEntry;
use crate::utils::sync::IrqGuard;

static HOSTNAME: Once<Mutex<String>> = Once::new();

fn hostname() -> &'static Mutex<String> {
    HOSTNAME.call_once(|| Mutex::new(String::from("aero")))
}

#[aero_proc::syscall]
pub fn exit(status: usize) -> ! {
    #[cfg(all(test, feature = "ci"))]
    crate::emu::exit_qemu(crate::emu::ExitStatus::Success);

    #[cfg(not(feature = "ci"))]
    {
        log::trace!(
            "exiting the process (pid={pid}) with status: {status}",
            pid = scheduler::get_scheduler().current_task().pid().as_usize(),
            status = status
        );

        scheduler::get_scheduler().exit(status as isize);
    }
}

#[aero_proc::syscall]
pub fn uname(buffer: &mut Utsname) -> Result<usize, AeroSyscallError> {
    fn init_array(fixed: &mut [u8; 65], init: &'static str) {
        let init_bytes = init.as_bytes();
        let len = init.len();

        fixed[..len].copy_from_slice(init_bytes)
    }

    init_array(&mut buffer.name, "Aero");
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

#[aero_proc::syscall]
pub fn fork() -> Result<usize, AeroSyscallError> {
    let scheduler = scheduler::get_scheduler();
    let forked = scheduler.current_task().fork();

    scheduler.register_task(forked.clone());
    Ok(forked.pid().as_usize())
}

#[aero_proc::syscall]
pub fn clone(entry: usize, stack: usize) -> Result<usize, AeroSyscallError> {
    let scheduler = scheduler::get_scheduler();
    let cloned = scheduler.current_task().clone_process(entry, stack);

    scheduler.register_task(cloned.clone());
    Ok(cloned.pid().as_usize())
}

#[aero_proc::syscall]
pub fn exec(
    path: &Path,
    args: usize,
    argc: usize,
    envs: usize,
    envc: usize,
) -> Result<usize, AeroSyscallError> {
    let executable = fs::lookup_path(path)?;

    if executable.inode().metadata()?.is_directory() {
        return Err(AeroSyscallError::EISDIR);
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
        .exec(executable, argv, envv)
        .expect("task: failed to exec task");

    unreachable!()
}

#[aero_proc::syscall]
pub fn log(msg: &str) -> Result<usize, AeroSyscallError> {
    log::debug!("{}", msg);

    Ok(0x00)
}

#[aero_proc::syscall]
pub fn waitpid(pid: usize, status: &mut u32, _flags: usize) -> Result<usize, AeroSyscallError> {
    let current_task = scheduler::get_scheduler().current_task();

    Ok(current_task.waitpid(pid as isize, status)?)
}

#[aero_proc::syscall]
pub fn mmap(
    address: usize,
    size: usize,
    protection: usize,
    flags: usize,
    fd: usize,
    offset: usize,
) -> Result<usize, AeroSyscallError> {
    let address = VirtAddr::new(address as u64);
    let protection = MMapProt::from_bits(protection).ok_or(AeroSyscallError::EINVAL)?;
    let flags = MMapFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;

    let mut file = None;

    if fd as isize != -1 {
        file = Some(
            scheduler::get_scheduler()
                .current_task()
                .file_table
                .get_handle(fd)
                .ok_or(AeroSyscallError::EBADF)?
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
        Err(AeroSyscallError::EFAULT)
    }
}

#[aero_proc::syscall]
pub fn munmap(address: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let address = VirtAddr::new(address as u64);

    if scheduler::get_scheduler()
        .current_task()
        .vm
        .munmap(address, size)
    {
        Ok(0x00)
    } else {
        Err(AeroSyscallError::EFAULT)
    }
}

#[aero_proc::syscall]
pub fn getpid() -> Result<usize, AeroSyscallError> {
    Ok(scheduler::get_scheduler().current_task().pid().as_usize())
}

#[aero_proc::syscall]
pub fn gettid() -> Result<usize, AeroSyscallError> {
    Ok(scheduler::get_scheduler().current_task().tid().as_usize())
}

#[aero_proc::syscall]
pub fn gethostname(buffer: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let hostname = hostname().lock();
    let bytes = hostname.as_bytes();

    if bytes.len() > buffer.len() {
        Err(AeroSyscallError::ENAMETOOLONG)
    } else {
        buffer[0..bytes.len()].copy_from_slice(bytes);

        Ok(bytes.len())
    }
}

#[aero_proc::syscall]
pub fn info(struc: &mut SysInfo) -> Result<usize, AeroSyscallError> {
    struc.uptime = crate::time::get_uptime_ticks() as i64;

    Ok(0x00)
}

#[aero_proc::syscall]
pub fn sethostname(name: &[u8]) -> Result<usize, AeroSyscallError> {
    match core::str::from_utf8(name) {
        Ok(name) => {
            *hostname().lock() = name.into();

            Ok(0)
        }
        Err(_) => Err(AeroSyscallError::EINVAL),
    }
}

#[aero_proc::syscall]
pub fn sigprocmask(
    how: usize,
    set: *const u64,
    old_set: *mut u64,
) -> Result<usize, AeroSyscallError> {
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

#[aero_proc::syscall]
pub fn sigaction(
    sig: usize,
    sigact: *mut SigAction,
    sigreturn: usize,
    old: *mut SigAction,
) -> Result<usize, AeroSyscallError> {
    let new = if sigact.is_null() {
        None
    } else {
        Some(unsafe { &mut *sigact })
    };

    let entry = if let Some(new) = new {
        Some(SignalEntry::from_sigaction(*new, sigreturn)?)
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

#[aero_proc::syscall]
pub fn shutdown() -> ! {
    fs::cache::dcache().log();

    fs::cache::clear_inode_cache();
    fs::cache::clear_dir_cache();

    let _guard = IrqGuard::new();
    lai::enter_sleep(5);

    unreachable!("aml: failed to shutdown (enter state S5)")
}
