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

use crate::arch::gdt::TASK_STATE_SEGMENT;
use crate::mem::paging::VirtAddr;
use crate::userland::scheduler;
use crate::utils::io;
use crate::utils::linker::LinkerSymbol;

struct Tls {
    sptr: VirtAddr,
}

impl Tls {
    fn new(address: VirtAddr) -> &'static mut Tls {
        unsafe { &mut *address.as_mut_ptr::<Tls>() }
    }

    fn setup(&mut self) {
        self.sptr = VirtAddr::new(self as *mut _ as u64);

        unsafe {
            io::wrmsr(io::IA32_FS_BASE, self.sptr.as_u64());
        }
    }
}

/// Initialize support for the `#[thread_local]` attribute.
pub fn init() {
    extern "C" {
        /// The starting byte of the thread data segment.
        static __tdata_start: LinkerSymbol;
        /// The ending byte of the thread data segment.
        static __tdata_end: LinkerSymbol;
    }

    let total_size = unsafe { __tdata_end.as_usize() - __tdata_start.as_usize() };

    let tls_layout = unsafe { Layout::from_size_align_unchecked(total_size + 8, 8) };
    let tls_raw_ptr = unsafe { alloc_zeroed(tls_layout) };

    unsafe {
        __tdata_start.as_ptr().copy_to(tls_raw_ptr, total_size);

        let tls = Tls::new(VirtAddr::new(tls_raw_ptr.add(total_size) as u64));
        tls.setup();

        // SAFETY: Safe to access thread local variables as at this point are accessible.
        TASK_STATE_SEGMENT.set_kernel_fs(tls.sptr.as_u64());
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
