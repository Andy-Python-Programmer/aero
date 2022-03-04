use raw_cpuid::CpuId;

use crate::arch::gdt::GdtEntryType;
use crate::utils::io;

use super::interrupts::InterruptStack;

extern "C" {
    fn x86_64_syscall_handler();
}

#[no_mangle]
extern "C" fn x86_64_do_syscall(stack: &mut InterruptStack) {
    let syscall_number = stack.scratch.rax as usize;

    match syscall_number {
        aero_syscall::prelude::SYS_EXIT => {}
        aero_syscall::prelude::SYS_SIGRETURN => {
            let result = super::signals::sigreturn(stack);
            stack.scratch.rax = result as u64;
            return;
        }

        _ => unsafe { super::interrupts::enable_interrupts() },
    }

    let result_usize = crate::syscall::generic_do_syscall(
        syscall_number,
        stack.scratch.rdi as usize, // argument 1
        stack.scratch.rsi as usize, // argument 2
        stack.scratch.rdx as usize, // argument 3
        stack.scratch.r10 as usize, // argument 4
        stack.scratch.r8 as usize,  // argument 5
        stack.scratch.r9 as usize,  // argument 6
    );

    super::signals::syscall_check_signals(result_usize as isize, stack);
    stack.scratch.rax = result_usize as _;
}

/// Initializes support for the `syscall` and `sysret` instructions for the
/// current CPU.
pub(super) fn init() {
    // Check if syscall is supported as it is a required CPU feature for aero to run.
    let has_syscall = CpuId::new()
        .get_extended_processor_and_feature_identifiers()
        .map_or(false, |i| i.has_syscall_sysret());

    assert!(has_syscall);

    unsafe {
        /*
         * Enable support for `syscall` and `sysret` instructions if the current
         * CPU supports them and the target pointer width is 64.
         */
        let syscall_base = GdtEntryType::KERNEL_CODE << 3;
        let sysret_base = (GdtEntryType::USER_CODE32_UNUSED << 3) | 3;

        let star_hi = syscall_base as u32 | ((sysret_base as u32) << 16);

        io::wrmsr(io::IA32_STAR, (star_hi as u64) << 32);
        io::wrmsr(io::IA32_LSTAR, x86_64_syscall_handler as u64);

        // Clear the trap flag and enable interrupts.
        io::wrmsr(io::IA32_FMASK, 0x300);

        // Set the EFER.SCE bit to enable the syscall feature
        let efer = io::rdmsr(io::IA32_EFER);
        io::wrmsr(io::IA32_EFER, efer | 1);
    }
}
