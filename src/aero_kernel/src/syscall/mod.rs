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

//! System Calls are used to call a kernel service from user land.
//!
//! | %rax   | Name                    |
//! |--------|-------------------------|
//! | 0      | read                    |
//! | 1      | write                   |
//! | 2      | open                    |
//! | 3      | close                   |
//! | 60     | exit                    |
//!
//! **Notes**: <https://wiki.osdev.org/System_Calls>

use aero_syscall::prelude::*;

pub mod fs;
pub mod process;
pub mod time;

pub use fs::*;
pub use process::*;
pub use time::*;

use crate::arch::interrupts::interrupt_stack;
use crate::arch::{gdt::GdtEntryType, interrupts};

use crate::utils::io;

#[repr(C)]
struct SyscallFrame {
    rflags: u64,
    rip: u64,
    rsp: u64,
}

#[repr(C)]
struct RegistersFrame {
    cr2: u64,
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}

#[no_mangle]
extern "C" fn __inner_syscall(_sys: &mut SyscallFrame, stack: &mut RegistersFrame) {
    let a = stack.rax as usize;
    let b = stack.rdi as usize;
    let c = stack.rsi as usize;
    let d = stack.rdx as usize;
    let e = stack.r10 as usize;
    // let f = scratch.r8;

    match a {
        SYS_EXIT => {}
        _ => unsafe { interrupts::enable_interrupts() },
    }

    let result = match a {
        SYS_EXIT => process::exit(b),
        SYS_SHUTDOWN => process::shutdown(),
        SYS_OPEN => fs::open(b, c, d, e),
        SYS_WRITE => fs::write(b, c, d),
        _ => {
            log::error!("Invalid syscall: {:#x}", a);

            Err(AeroSyscallError::ENOSYS)
        }
    };

    stack.rax = aero_syscall::syscall_result_as_usize(result) as _;
}

interrupt_stack!(
    pub unsafe fn syscall_interrupt_handler(stack: &mut InterruptStack) {
        if supports_syscall_sysret() {
            // If the current CPU suppots syscall instruction then print
            // a warning as in this case use of deperecated `int 0x80`
            // interrupt.
            log::warn!("Use of deperecated `int 0x80` interrupt");
        }

        unimplemented!()
    }
);
extern "C" {
    fn syscall_handler();
}

pub fn init() {
    unsafe {
        /*
         * Enable support for `syscall` and `sysret` instructions if the current
         * CPU supports them and the target pointer width is 64.
         */
        #[cfg(target_pointer_width = "64")]
        if supports_syscall_sysret() {
            let syscall_base = GdtEntryType::KERNEL_CODE << 3;
            let sysret_base = (GdtEntryType::USER_CODE32_UNUSED << 3) | 3;

            let star_hi = syscall_base as u32 | ((sysret_base as u32) << 16);

            io::wrmsr(io::IA32_STAR, (star_hi as u64) << 32);
            io::wrmsr(io::IA32_LSTAR, syscall_handler as u64);

            // Clear the trap flag and enable interrupts.
            io::wrmsr(io::IA32_FMASK, 0x300);

            let efer = io::rdmsr(io::IA32_EFER);
            io::wrmsr(io::IA32_EFER, efer | 1);
        }
    }
}
