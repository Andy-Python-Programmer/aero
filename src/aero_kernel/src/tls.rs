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

//! Thread Local Storage (TLS) are per-thread global variables. On 64-bit each CPU core's
//! `fs` GDT segment points to the thread local memory area where the thread local static's
//! live. TLS statics are simply accessed through an offset from `fs`.
//!
//! ## Notes
//! * <https://wiki.osdev.org/Thread_Local_Storage>
//! * <https://doc.rust-lang.org/std/thread/struct.LocalKey.html>

use core::alloc::Layout;

use alloc::alloc::alloc_zeroed;

use crate::arch::gdt::{Kpcr, Tss};
use crate::userland::scheduler;
use crate::utils::io;

pub struct PerCpuData {
    pub cpuid: usize,
}

/// SAFETY: The GS base should point to the kernel PCR.
pub fn get_cpuid() -> usize {
    get_percpu().cpuid
}

/// SAFETY: The GS base should point to the kernel PCR.
pub fn get_percpu() -> &'static mut PerCpuData {
    unsafe { (&mut *(io::rdmsr(io::IA32_GS_BASE) as *mut Kpcr)).cpu_local }
}

pub fn init() {
    let size = core::mem::size_of::<PerCpuData>();

    // NOTE: Inside kernel space, the GS base will always point to the CPU local data and when
    // jumping to userland `swapgs` is called making the GS base point to the userland TLS data.
    unsafe {
        let tss_layout = Layout::from_size_align_unchecked(
            core::mem::size_of::<Kpcr>(),
            core::mem::align_of::<Kpcr>(),
        );

        let tss_ptr = alloc_zeroed(tss_layout) as *mut Tss;
        io::wrmsr(io::IA32_GS_BASE, tss_ptr as u64);

        let tls_layout = Layout::from_size_align_unchecked(size, 8);
        let tls_raw_ptr = alloc_zeroed(tls_layout);

        crate::arch::gdt::get_kpcr().cpu_local = &mut *(tls_raw_ptr as *mut PerCpuData);
    }
}

#[no_mangle]
extern "C" fn restore_user_tls() {
    unsafe {
        let base = scheduler::get_scheduler()
            .current_task()
            .arch_task_mut()
            .get_fs_base();

        io::wrmsr(io::IA32_FS_BASE, base.as_u64());
    }
}
