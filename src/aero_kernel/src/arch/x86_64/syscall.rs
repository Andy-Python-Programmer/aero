use aero_syscall::AeroSyscallError;
use raw_cpuid::CpuId;

use crate::arch::gdt::GdtEntryType;
use crate::mem::paging::VirtAddr;
use crate::userland::scheduler;
use crate::utils::io;

use super::interrupts::InterruptErrorStack;

extern "C" {
    fn x86_64_syscall_handler();
}

const ARCH_SET_GS: usize = 0x1001;
const ARCH_SET_FS: usize = 0x1002;
const ARCH_GET_FS: usize = 0x1003;
const ARCH_GET_GS: usize = 0x1004;

fn arch_prctl(command: usize, address: usize) -> Result<usize, AeroSyscallError> {
    match command {
        ARCH_SET_FS => unsafe {
            scheduler::get_scheduler()
                .current_task()
                .arch_task_mut()
                .set_fs_base(VirtAddr::new(address as u64));

            Ok(0x00)
        },

        ARCH_GET_FS => Ok(scheduler::get_scheduler()
            .current_task()
            .arch_task()
            .get_fs_base()
            .as_u64() as usize),

        ARCH_SET_GS => unsafe {
            scheduler::get_scheduler()
                .current_task()
                .arch_task_mut()
                .set_gs_base(VirtAddr::new(address as u64));

            Ok(0x00)
        },

        ARCH_GET_GS => Ok(scheduler::get_scheduler()
            .current_task()
            .arch_task()
            .get_gs_base()
            .as_u64() as usize),

        _ => Err(AeroSyscallError::EINVAL),
    }
}

#[no_mangle]
extern "C" fn x86_64_do_syscall(stack: &mut InterruptErrorStack) {
    let stack = &mut stack.stack;

    let syscall_number = stack.scratch.rax as usize; // syscall number
    let a = stack.scratch.rdi as usize; // argument 1
    let b = stack.scratch.rsi as usize; // argument 2
    let c = stack.scratch.rdx as usize; // argument 3
    let d = stack.scratch.r10 as usize; // argument 4
    let e = stack.scratch.r8 as usize; // argument 5
    let f = stack.scratch.r9 as usize; // argument 6

    match syscall_number {
        // handle arch-specific syscalls (`sigreturn` and `arch_prctl`):
        aero_syscall::prelude::SYS_SIGRETURN => {
            let result = super::signals::sigreturn(stack);
            stack.scratch.rax = result as u64;
            return;
        }

        aero_syscall::prelude::SYS_ARCH_PRCTL => {
            let result = self::arch_prctl(a, b);
            let result_usize = aero_syscall::syscall_result_as_usize(result);

            stack.scratch.rax = result_usize as _;
            return;
        }

        aero_syscall::prelude::SYS_EXIT => {}
        _ => unsafe { super::interrupts::enable_interrupts() },
    }

    let result_usize = crate::syscall::generic_do_syscall(syscall_number, a, b, c, d, e, f);

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
