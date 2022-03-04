use raw_cpuid::CpuId;

use crate::arch::gdt::GdtEntryType;
use crate::utils::io;

extern "C" {
    fn x86_64_syscall_handler();
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
