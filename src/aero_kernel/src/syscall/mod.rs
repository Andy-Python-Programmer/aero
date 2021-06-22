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

use crate::arch::interrupts::{interrupt_stack, InterruptStack};
use crate::arch::{gdt::GdtEntryType, interrupts};

use crate::prelude::*;
use crate::utils::io;

fn __inner_syscall(stack: &mut InterruptStack) -> usize {
    let scratch = &stack.scratch;

    let a = scratch.rax as usize;
    let b = scratch.rdi as usize;
    let c = scratch.rsi as usize;
    let d = scratch.rdx as usize;
    // let e = scratch.r10;
    // let f = scratch.r8;

    match a {
        SYS_EXIT => {}
        _ => unsafe { interrupts::enable_interrupts() },
    }

    let result = match a {
        SYS_EXIT => process::exit(b),
        SYS_SHUTDOWN => process::shutdown(),
        SYS_OPEN => fs::open(b, c, d),
        SYS_WRITE => fs::write(b, c, d),
        _ => {
            log::error!("Invalid syscall: {:#x}", a);

            Err(AeroSyscallError::ENOSYS)
        }
    };

    aero_syscall::syscall_result_as_usize(result)
}

#[no_mangle]
unsafe extern "C" fn __impl_syscall_handler(stack: *mut InterruptStack) {
    let stack = &mut *stack;
    let result = __inner_syscall(stack);

    (*stack).scratch.rax = result as _;
}

interrupt_stack!(
    pub unsafe fn syscall_interrupt_handler(stack: &mut InterruptStack) {
        if supports_syscall_sysret() {
            // If the current CPU suppots syscall instruction then print
            // a warning as in this case use of deperecated `int 0x80`
            // interrupt.
            log::warn!("Use of deperecated `int 0x80` interrupt");
        }

        let result = __inner_syscall(stack);
        (*stack).scratch.rax = result as _;
    }
);

intel_fn! {
    #![cfg(target_pointer_width = "64")]

    /**
     * 64-bit `syscall` instruction handler.
     *
     * The `syscall` instruction should only be used for 64-bit system calls. The
     * `syscall` instruction saves RIP to RAX, clears rflags.RF, saves rflags
     * to R11 and then loads new SS, CS and RIP from previously programmed MSRs.
     *
     * ## Saftey
     * The syscall instruction should only be called in usermode/ring 3. If not you will
     * be pleased by a page fault :D
     */
    pub extern "asm" fn syscall_handler() {
        "swapgs\n", // Set gs segment to TSS.

        "mov gs:[0x1C], rsp\n", // Save userspace stack pointer.
        "mov rsp, gs:[0x04]\n", // Load kernel stack pointer.

        "push QWORD PTR 5 * 8 + 3\n", // Push fake userspace SS resembling `iret` frame.

        "push QWORD PTR gs:[0x1C]\n", // Push userspace rsp.
        "push r11\n", // Push rflags in r11.

        "push QWORD PTR 6 * 8 + 3\n", // Push fake CS resembling `iret` stack frame.
        "push rcx\n", // Push userspace return pointer.

        "call restore_kernel_tls\n", // Restore the kernel thread local storage.

        "push rax\n",
        crate::prelude::push_scratch!(),
        crate::prelude::push_preserved!(),
        crate::prelude::push_fs!(),

        "mov rdi, rsp\n",
        "call __impl_syscall_handler\n", // Call the inner syscall handler function.

        "cli\n", // Renabled after `sysretq`. To be safe as we are doing `swapfs`.
        "call restore_user_tls\n", // Restore the userland thread local storage.

        crate::prelude::pop_fs!(),
        crate::prelude::pop_preserved!(),
        crate::prelude::pop_scratch!(),

        /*
         * In Intel CPUs, `sysretq` with non-canonical RCX or RIP will cause
         * a general protection fault in kernel space. This lets the user take over
         * the kernel, since userland controls RSP.
         *
         * So set the ZF iff forbidden bits 63:47 (i.e. the bits that must be sign extended) of the
         * pushed RCX are set. Then do a conditional jump to slow `sysretq` if ZF was set then the
         * address had an invalid higher half. This prevents execution **possibly** of attacker controlled
         * code.
         */
        "test DWORD PTR [rsp + 4], 0xFFFF8000\n",
        "jnz 1f\n",

        "pop rcx\n", // Pop userspace return pointer.
        "add rsp, 8\n", // Pop fake userspace CS.

        "pop r11\n", // Pop rflags in r11.
        "pop QWORD PTR gs:[0x1C]\n", // Pop userspace stack pointer.

        "mov rsp, gs:[0x1C]\n", // Restore userspace stack pointer.
        "swapgs\n", // Restore gs from TSS to user data.

        "sysretq\n", // Return back into userspace.
    }

    pub extern "asm" 1 => {
        "xor rcx, rcx\n",
        "xor r11, r11\n",
        "swapgs\n",
        "iretq\n",
    }
}

#[repr(C)]
pub struct SyscallFrame {
    pub rflags: u64,
    pub rip: u64,
    pub rsp: u64,
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
