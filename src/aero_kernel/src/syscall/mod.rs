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

use crate::arch::interrupts::{interrupt, InterruptStack};
use crate::arch::{gdt::GdtEntryType, interrupts};

use crate::prelude::*;
use crate::utils::io;

fn __inner_syscall(stack: &mut InterruptStack) -> usize {
    let scratch = &stack.scratch;

    let a = scratch.rax;
    let b = scratch.rdi;
    let c = scratch.rsi;
    let d = scratch.rdx;
    let e = scratch.r10;
    let f = scratch.r8;

    match a {
        SYS_EXIT => {}
        _ => unsafe { interrupts::enable_interrupts() },
    }

    let result = match a {
        SYS_EXIT => process::exit(b),
        _ => {
            log::error!("Invalid syscall: {:#x}", a);

            Err(AeroSyscallError::Unknown)
        }
    };

    aero_syscall::syscall_result_as_usize(result)
}

#[no_mangle]
unsafe extern "C" fn __impl_syscall_handler(stack: *mut InterruptStack) {
    let stack = &mut *stack;
    let result = __inner_syscall(stack);

    (*stack).scratch.rax = result;
}

interrupt!(
    pub unsafe fn syscall_interrupt_handler(stack: &mut InterruptStack) {
        if supports_syscall_sysret() {
            // If the current CPU suppots syscall instruction then print
            // a warning as in this case use of deperecated `int 0x80`
            // interrupt.
            log::warn!("Use of deperecated `int 0x80` interrupt");
        }

        let result = __inner_syscall(stack);
        (*stack).scratch.rax = result;
    }
);

intel_fn!(
    #![cfg(target_pointer_width = "64")]

    /**
     * 64-bit `syscall` instruction handler.
     *
     * The `syscall` instruction should only be used for 64-bit system calls. The
     * `syscall` instruction saves RIP to RAX, clears rflags.RF, saves rflags
     * to R11 and then loads new SS, CS and RIP from previously programmed MSRs.
     */
    pub extern "asm" fn syscall_handler() {
        "swapgs\n", // Set gs segment to TSS.

        "mov gs:[0x08], rsp\n", // Save userspace stack pointer.
        "mov rsp, gs:[0x14]\n", // Load kernel stack pointer.

        "push QWORD PTR 5 * 8 + 3\n", // Push fake userspace SS resembling `iret` frame.

        "push QWORD PTR gs:[0x08]\n", // Push userspace rsp.
        "push r11\n", // Push rflags in r11.

        "push QWORD PTR 6 * 8 + 3\n", // Push fake CS resembling `iret` stack frame.
        "push rcx\n", // Push userspace return pointer.

        "push rax\n",
        crate::prelude::push_scratch!(),
        crate::prelude::push_preserved!(),

        "mov rdi, rsp\n",
        "call __impl_syscall_handler\n", // Call the inner syscall handler function.

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
        "pop QWORD PTR gs:[0x08]\n", // Pop userspace stack pointer.

        "mov rsp, gs:[0x08]\n", // Restore userspace stack pointer.
        "swapgs\n", // Restore gs from TSS to user data.

        "sysretq\n", // Return back into userspace.
    }

    pub extern "asm" 1 => {
        "xor rcx, rcx\n",
        "xor r11, r11\n",
        "swapgs\n",
        "iretq\n",
    }
);

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
