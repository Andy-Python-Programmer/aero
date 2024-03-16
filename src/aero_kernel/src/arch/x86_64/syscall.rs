use aero_syscall::SyscallError;
use raw_cpuid::CpuId;

use crate::arch::gdt::{GdtEntryType, Tss, USER_CS, USER_SS};
use crate::mem::paging::VirtAddr;
use crate::userland::scheduler::{self, ExitStatus};
use crate::utils::sync::IrqGuard;

use super::interrupts::InterruptErrorStack;
use super::{asm_macros, io};

use core::mem::offset_of;

const ARCH_SET_GS: usize = 0x1001;
const ARCH_SET_FS: usize = 0x1002;
const ARCH_GET_FS: usize = 0x1003;
const ARCH_GET_GS: usize = 0x1004;

/// 64-bit SYSCALL instruction entry point.
///
/// The instruction supports to to 6 arguments in registers.
///
/// Registers state on entry:
/// * `RAX` - system call number
/// * `RCX` - return address
/// * `R11` - saved flags (note: R11 is callee-clobbered register in C ABI)
/// * `RDI` - argument 1
/// * `RSI` - argument 2
/// * `RDX` - argument 3
/// * `R10` - argument 4 (needs to be moved to RCX to conform to C ABI)
/// * `R8`  - argument 5
/// * `R9`  - argument 6
///
/// (note: `R12`..`R15`, `RBP`, `RBX` are callee-preserved in C ABI)
///
/// The instruction saves the `RIP` to `RCX`, clears `RFLAGS.RF` then saves `RFLAGS` to `R11`.
/// Followed by, it loads the new `SS`, `CS`, and `RIP` from previously programmed MSRs.
///
/// The instruction also does not save anything on the stack and does *not* change the `RSP`.
#[naked]
unsafe extern "C" fn x86_64_syscall_handler() {
    asm!(
        // make the GS base point to the kernel TLS
        "swapgs",
        // save the user stack pointer
        "mov qword ptr gs:{tss_temp_ustack_off}, rsp",
        // restore kernel stack
        "mov rsp, qword ptr gs:{tss_rsp0_off}",
        "push {userland_ss}",
        // push userspace stack ptr
        "push qword ptr gs:{tss_temp_ustack_off}",
        "push r11",
        "push {userland_cs}",
        "push rcx",

        "push rax",
        asm_macros::push_scratch!(),
        asm_macros::push_preserved!(),

        // push a fake error code to match with the layout of `InterruptErrorStack`
        "push 0",

        "mov rdi, rsp",

        "cld",
        "call {x86_64_do_syscall}",
        "cli",

        // pop the fake error code
        "add rsp, 8",

        asm_macros::pop_preserved!(),
        asm_macros::pop_scratch!(),

        // cook the sysret frame
        "pop rcx",
        "add rsp, 8",
        "pop r11",
        "pop rsp",

        // restore user GS
        "swapgs",
        "sysretq",

        // constants:
        userland_cs = const USER_CS.bits(),
        userland_ss = const USER_SS.bits(),
        // XXX: add 8 bytes to skip the x86_64 cpu local self ptr
        tss_temp_ustack_off = const offset_of!(Tss, reserved2) + core::mem::size_of::<usize>(),
        tss_rsp0_off = const offset_of!(Tss, rsp) + core::mem::size_of::<usize>(),
        x86_64_do_syscall = sym x86_64_do_syscall,
        options(noreturn)
    )
}

/// 64-bit SYSENTER instruction entry point.
///
/// The SYSENTER mechanism performs a fast transition to the kernel.
///
/// The new `CS` is loaded from the `IA32_SYSENTER_CS` MSR, and the new instruction and stack
/// pointers are loaded from `IA32_SYSENTER_EIP` and `IA32_SYSENTER_ESP`, respectively. `RFLAGS.IF`
/// is cleared, but other flags are unchanged.
///
/// As the instruction does not save *any* state, the user is required to provide the return `RIP`
/// and `RSP` in the `RCX` and `R11` registers, respectively. These addresses must be canonical.
///
/// The instruction expects the call number and arguments in the same registers as for SYSCALL.
#[naked]
unsafe extern "C" fn x86_64_sysenter_handler() {
    asm!(
        "swapgs",
        // Build the interrupt frame expected by the kernel.
        "push {userland_ss}",
        "push r11",
        "pushfq",
        "push {userland_cs}",
        "push rcx",
        // Mask the same flags as for SYSCALL.
        // XXX: Up to this point the code can be single-stepped if the user sets TF.
        "pushfq",
        "and dword ptr [rsp], 0x300",
        "popfq",
        "push rax",
        asm_macros::push_scratch!(),
        asm_macros::push_preserved!(),
        "push 0",
        // Store the stack pointer (interrupt frame ptr) in `RBP` for safe keeping, and align the
        // stack as specified by the SysV calling convention.
        "mov rbp, rsp",
        "and rsp, ~0xf",
        "mov rdi, rbp",
        "call {x86_64_check_sysenter}",
        "mov rdi, rbp",
        "call {x86_64_do_syscall}",
        // Reload the stack pointer, skipping the error code.
        "lea rsp, [rbp + 8]",
        asm_macros::pop_preserved!(),
        asm_macros::pop_scratch!(),
        // Pop the `IRET` frame into the registers expected by `SYSEXIT`.
        "pop rdx", // return `RIP` in `RDX`
        "add rsp, 8",
        "popfq",   // restore saved `RFLAGS`
        "pop rcx", // return `RSP` in `RCX`
        // SAFETY: The above call to `x86_64_check_sysenter` is guarantees that we execute
        // `sysexit` with canonical addresses in RCX and RDX. Otherwise we would fault in the
        // kernel having already swapped back to the user's GS.
        "swapgs",
        // SYSEXIT does *not* restore `IF` to re-enable interrupts.
        // This is done here, rather then when restoring `RFLAGS` above, since `STI` will keep
        "sti",
        // interrupts inhibited until after the *following* instruction executes.
        "sysexitq",
        // constants:
        userland_cs = const USER_CS.bits(),
        userland_ss = const USER_SS.bits(),
        x86_64_check_sysenter = sym x86_64_check_sysenter,
        x86_64_do_syscall = sym x86_64_do_syscall,
        options(noreturn)
    )
}

fn arch_prctl(command: usize, address: usize) -> Result<usize, SyscallError> {
    match command {
        ARCH_SET_FS => unsafe {
            let _guard = IrqGuard::new();

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
            let _guard = IrqGuard::new();

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

        _ => Err(SyscallError::EINVAL),
    }
}

/// Check the user-provided return addresses for system calls via SYSENTER
///
/// We cannot execute `sysexit` on return with non-canonical return addresses, or we
/// will take a fault in the kernel with the user's GS base already swapped back.
pub(super) extern "sysv64" fn x86_64_check_sysenter(stack: &mut InterruptErrorStack) {
    let rip = stack.stack.iret.rip;
    let rsp = stack.stack.iret.rsp;
    let max_user_addr = super::task::userland_last_address().as_u64();

    if rip > max_user_addr || rsp > max_user_addr {
        log::error!("bad sysenter: rip={:#018x},rsp={:#018x}", rip, rsp);

        // Forcibly kill the process, we have nowhere to return to.
        scheduler::get_scheduler().exit(ExitStatus::Normal(-1));
    }
}

pub(super) extern "C" fn x86_64_do_syscall(stack: &mut InterruptErrorStack) {
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
            super::signals::sigreturn(stack);
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
    let cpuid = CpuId::new();

    // Check if syscall is supported as it is a required CPU feature for aero to run.
    let has_syscall = cpuid
        .get_extended_processor_and_feature_identifiers()
        .map_or(false, |i| i.has_syscall_sysret());

    assert!(has_syscall);

    unsafe {
        // Enable support for `syscall` and `sysret` instructions if the current
        // CPU supports them and the target pointer width is 64.
        let syscall_base = GdtEntryType::KERNEL_CODE << 3;
        let sysret_base = (GdtEntryType::KERNEL_TLS << 3) | 3;

        let star_hi = syscall_base as u32 | ((sysret_base as u32) << 16);

        io::wrmsr(io::IA32_STAR, (star_hi as u64) << 32);
        io::wrmsr(io::IA32_LSTAR, x86_64_syscall_handler as u64);

        // Clear the trap flag and enable interrupts.
        io::wrmsr(io::IA32_FMASK, 0x300);

        // Set the EFER.SCE bit to enable the syscall feature
        let efer = io::rdmsr(io::IA32_EFER);
        io::wrmsr(io::IA32_EFER, efer | 1);
    }

    // Enable support for the `sysenter` instruction for system calls.
    //
    // This instruction is only supported on AMD processors in Legacy mode,
    // not in Long mode (Compatibility or 64-bit modes), so still report support
    // for it via `cpuid`. In this case the #UD exception is caught to handle the
    // system call.
    let has_sysenter = cpuid
        .get_feature_info()
        .map_or(false, |i| i.has_sysenter_sysexit());

    if has_sysenter {
        log::info!("enabling support for sysenter");
        unsafe {
            io::wrmsr(
                io::IA32_SYSENTER_CS,
                (GdtEntryType::KERNEL_CODE << 3) as u64,
            );
            io::wrmsr(io::IA32_SYSENTER_EIP, x86_64_sysenter_handler as u64);
        }
    }
}
