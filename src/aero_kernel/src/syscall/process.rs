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

use aero_syscall::{AeroSyscallError, AeroSyscallResult, MMapFlags, MMapProt};

use crate::mem::paging::VirtAddr;
use crate::userland::scheduler;

pub fn exit(status: usize) -> ! {
    log::trace!("Exiting the current process with status: {}", status);
    scheduler::exit_current_task(status);
}

pub fn fork() -> AeroSyscallResult {
    let scheduler = scheduler::get_scheduler();
    let forked = scheduler.current_task().fork();

    scheduler.register_task(forked.clone());
    Ok(forked.task_id().as_usize())
}

pub fn mmap(
    address: usize,
    size: usize,
    protocol: usize,
    flags: usize,
    _fd: usize,
    _offset: usize,
) -> AeroSyscallResult {
    let address = VirtAddr::new(address as u64);

    let protocol = MMapProt::from_bits(protocol).ok_or(AeroSyscallError::EINVAL)?;
    let flags = MMapFlags::from_bits(flags).ok_or(AeroSyscallError::EINVAL)?;

    if !flags.contains(MMapFlags::MAP_ANONYOMUS) {
        unimplemented!()
    }

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

pub fn shutdown() -> ! {
    crate::fs::cache::clear_inode_cache();
    // TODO
    loop {}
}
