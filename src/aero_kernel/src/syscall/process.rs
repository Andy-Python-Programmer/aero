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

use aero_syscall::signal::SigAction;
use aero_syscall::{AeroSyscallError, MMapFlags, MMapProt};
use alloc::string::String;
use spin::{Mutex, Once};

use crate::fs;
use crate::fs::Path;

use crate::mem::paging::VirtAddr;
use crate::userland::scheduler;
use crate::userland::signals::SignalEntry;
use crate::utils::validate_str;

static HOSTNAME: Once<Mutex<String>> = Once::new();

fn hostname() -> &'static Mutex<String> {
    HOSTNAME.call_once(|| Mutex::new(String::from("aero")))
}

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

        scheduler::get_scheduler().inner.exit(status as isize);
    }
}

const ARCH_SET_GS: usize = 0x1001;
const ARCH_SET_FS: usize = 0x1002;
const ARCH_GET_FS: usize = 0x1003;
const ARCH_GET_GS: usize = 0x1004;

pub fn arch_prctl(command: usize, address: usize) -> Result<usize, AeroSyscallError> {
    match command {
        ARCH_SET_FS => {
            scheduler::get_scheduler()
                .current_task()
                .arch_task_mut()
                .set_fs_base(VirtAddr::new(address as u64));

            Ok(0x00)
        }

        ARCH_GET_FS => Ok(scheduler::get_scheduler()
            .current_task()
            .arch_task_mut()
            .get_fs_base()
            .as_u64() as usize),

        ARCH_SET_GS => unimplemented!(),
        ARCH_GET_GS => unimplemented!(),

        _ => Err(AeroSyscallError::EINVAL),
    }
}

pub fn uname(buffer: usize) -> Result<usize, AeroSyscallError> {
    fn init_array(fixed: &mut [u8; 65], init: &'static str) {
        let init_bytes = init.as_bytes();
        let len = init.len();

        fixed[..len].copy_from_slice(init_bytes)
    }

    // TODO: Safety checks!
    let struc = unsafe { &mut *(buffer as *mut aero_syscall::Utsname) };

    init_array(&mut struc.name, "Aero");
    init_array(&mut struc.nodename, "unknown");
    init_array(&mut struc.version, env!("CARGO_PKG_VERSION"));
    init_array(
        &mut struc.release,
        concat!(env!("CARGO_PKG_VERSION"), "-aero"),
    );

    #[cfg(target_arch = "x86_64")]
    init_array(&mut struc.machine, "x86_64");

    #[cfg(not(target_arch = "x86_64"))]
    init_array(&mut struc.machine, "unknown");

    Ok(0x00)
}

pub fn fork() -> Result<usize, AeroSyscallError> {
    let scheduler = scheduler::get_scheduler();
    let forked = scheduler.current_task().fork();

    scheduler.register_task(forked.clone());
    Ok(forked.pid().as_usize())
}

pub fn clone(entry: usize, stack: usize) -> Result<usize, AeroSyscallError> {
    let scheduler = scheduler::get_scheduler();
    let cloned = scheduler.current_task().clone_process(entry, stack);

    scheduler.register_task(cloned.clone());
    Ok(cloned.pid().as_usize())
}

pub fn exec(
    path: usize,
    path_size: usize,
    args: usize,
    argc: usize,
    envs: usize,
    envc: usize,
) -> Result<usize, AeroSyscallError> {
    let path = validate_str(path as *const u8, path_size).ok_or(AeroSyscallError::EINVAL)?;
    let path = Path::new(path);

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

pub fn log(msg_start: usize, msg_size: usize) -> Result<usize, AeroSyscallError> {
    let message = validate_str(msg_start as *const u8, msg_size).ok_or(AeroSyscallError::EINVAL)?;
    log::debug!("{}", message);

    Ok(0x00)
}

pub fn waitpid(pid: usize, status: usize, _flags: usize) -> Result<usize, AeroSyscallError> {
    let current_task = scheduler::get_scheduler().current_task();
    let status = unsafe { &mut *(status as *mut u32) };

    Ok(current_task.waitpid(pid, status)?)
}

pub fn mmap(
    address: usize,
    size: usize,
    protocol: usize,
    flags: usize,
    fd: usize,
    offset: usize,
) -> Result<usize, AeroSyscallError> {
    assert_eq!(offset as isize, 0);
    assert_eq!(fd as isize, -1);

    let address = VirtAddr::new(address as u64);

    let protocol = MMapProt::from_bits(protocol).ok_or(AeroSyscallError::EINVAL)?;
    let flags = MMapFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;

    if !flags.contains(MMapFlags::MAP_ANONYOMUS) {
        unimplemented!()
    }

    // HACK: This is currently a hack since mlibc tries to do somethin
    // fancy. Oh well andy plz fix this in the future.
    let size = size + 4096;

    if let Some(alloc) = scheduler::get_scheduler()
        .current_task()
        .vm()
        .mmap(address, size, protocol, flags)
    {
        Ok(alloc.as_u64() as usize)
    } else {
        Err(AeroSyscallError::EFAULT)
    }
}

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

pub fn getpid() -> Result<usize, AeroSyscallError> {
    Ok(scheduler::get_scheduler().current_task().pid().as_usize())
}

pub fn gettid() -> Result<usize, AeroSyscallError> {
    Ok(scheduler::get_scheduler().current_task().tid().as_usize())
}

pub fn gethostname(ptr: usize, length: usize) -> Result<usize, AeroSyscallError> {
    let slice = unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, length) };
    let hostname = hostname().lock();
    let bytes = hostname.as_bytes();

    if bytes.len() > slice.len() {
        Err(AeroSyscallError::ENAMETOOLONG)
    } else {
        slice[0..bytes.len()].copy_from_slice(bytes);

        Ok(bytes.len())
    }
}

pub fn info(struc: usize) -> Result<usize, AeroSyscallError> {
    let struc = unsafe { &mut *(struc as *mut aero_syscall::SysInfo) };

    // TODO: Fill in the rest of the struct.
    struc.uptime = crate::time::get_uptime_ticks() as i64;

    Ok(0x00)
}

pub fn sethostname(ptr: usize, length: usize) -> Result<usize, AeroSyscallError> {
    let slice = unsafe { core::slice::from_raw_parts(ptr as *const u8, length) };

    match core::str::from_utf8(slice) {
        Ok(new_hostname) => {
            *hostname().lock() = String::from(new_hostname);
            Ok(0)
        }
        Err(_) => Err(AeroSyscallError::EINVAL),
    }
}

pub fn sigaction(
    sig: usize,
    sigact: usize,
    sigreturn: usize,
    old: usize,
) -> Result<usize, AeroSyscallError> {
    if sig == 34 {
        // HECK: make mlibc happy :^)
        //
        // In function PthreadSignalInstaller, file mlibc/options/posix/generic/pthread-stubs.cpp:419
        return Err(AeroSyscallError::ENOSYS);
    }

    let new = if sigact == 0 {
        None
    } else {
        let address = VirtAddr::new(sigact as u64);
        let raw = address.as_mut_ptr::<SigAction>();
        let sigact = unsafe { &mut *raw };

        Some(sigact)
    };

    let entry = if let Some(new) = new {
        Some(SignalEntry::from_sigaction(*new, sigreturn)?)
    } else {
        None
    };

    let old = if old == 0 {
        None
    } else {
        let address = VirtAddr::new(old as u64);
        let raw = address.as_mut_ptr::<SigAction>();
        let sigact = unsafe { &mut *raw };

        Some(sigact)
    };

    let scheduler = scheduler::get_scheduler();
    let task = scheduler.current_task();
    let signals = task.signals();

    signals.set_signal(sig, entry, old);

    Ok(0)
}

pub fn shutdown() -> ! {
    fs::cache::dcache().log();

    fs::cache::clear_inode_cache();
    fs::cache::clear_dir_cache();

    // TODO
    loop {}
}
