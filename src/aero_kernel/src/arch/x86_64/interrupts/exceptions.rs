/*
 * Copyright (C) 2021-2023 The Aero Project Developers.
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

use super::{io, InterruptErrorStack};

use crate::arch::controlregs;
use crate::mem::paging::PageFaultErrorCode;

use crate::unwind;
use crate::userland::scheduler;

const LOG_PF_PTABLE: bool = true;

macro interrupt_exception(fn $name:ident() => $message:expr) {
    pub fn $name(stack: &mut InterruptErrorStack) {
        unwind::prepare_panic();

        log::error!("EXCEPTION: {}", $message);
        log::error!("Stack: {:#x?}", stack);

        unwind::unwind_stack_trace();

        unsafe {
            loop {
                super::halt();
            }
        }
    }
}

interrupt_exception!(fn divide_by_zero() => "Division by zero");
interrupt_exception!(fn debug() => "Debug");
interrupt_exception!(fn non_maskable() => "Non Maskable");
interrupt_exception!(fn overflow() => "Stack Overflow");
interrupt_exception!(fn bound_range() => "Out of Bounds");
interrupt_exception!(fn device_not_available() => "Device not Available");
interrupt_exception!(fn double_fault() => "Double Fault");
interrupt_exception!(fn invalid_tss() => "Invalid TSS");
interrupt_exception!(fn segment_not_present() => "Segment not Present");
interrupt_exception!(fn stack_segment() => "Stack Segment Fault");
interrupt_exception!(fn protection() => "Protection Fault");
interrupt_exception!(fn fpu_fault() => "FPU floating point fault");
interrupt_exception!(fn alignment_check() => "Alignment check fault");
interrupt_exception!(fn machine_check() => "Machine check fault");
interrupt_exception!(fn simd() => "SIMD floating point fault");
interrupt_exception!(fn virtualization() => "Virtualization fault");
interrupt_exception!(fn security() => "Security exception");

pub fn invalid_opcode(stack: &mut InterruptErrorStack) {
    // Catch SYSENTER on AMD CPUs.
    //
    // The RIP on the stack for #UD points to the instruction which generated the exception.
    // The return RIP and RSP need to be changed to the user-provided values in RCX and R11.
    const SYSENTER_OPCODE: [u8; 2] = [0x0f, 0x34];

    let opcode = unsafe { *(stack.stack.iret.rip as *const [u8; 2]) };
    if opcode == SYSENTER_OPCODE {
        log::debug!("handling SYSENTER via #UD");

        stack.stack.iret.rip = stack.stack.scratch.rcx;
        stack.stack.iret.rsp = stack.stack.scratch.r11;

        super::super::syscall::x86_64_check_sysenter(stack);
        super::super::syscall::x86_64_do_syscall(stack);
        return;
    }

    // Otherwise handle the exception as normal.

    unwind::prepare_panic();

    log::error!("EXCEPTION: Invalid Opcode");
    log::error!("Stack: {:#x?}", stack);

    unwind::unwind_stack_trace();

    unsafe {
        loop {
            super::halt();
        }
    }
}

pub fn breakpoint(stack: &mut InterruptErrorStack) {
    // We will need to prevent RIP from going out of sync with
    // instructions.
    //
    // So we will set the RIP to RIP - 1, pointing to the int3
    // instruction.
    (*stack).stack.iret.rip -= 1;
}

pub(super) fn page_fault(stack: &mut InterruptErrorStack) {
    let accessed_address = controlregs::read_cr2();
    let reason = PageFaultErrorCode::from_bits_truncate(stack.code);

    // We cannot directly check if we want to handle the page fault by checking
    // if the CS register contains the RPL_3 flag since, we also want to handle the
    // situation where we are trying to access a user provided buffer in the kernel and
    // its not mapped. So we handle the page fault if the accessed address is less then the
    // MAX userland address and we only signal kill the process if its trying to access
    // a non-mapped memory region while in RPL_3.
    let userland_last_address = super::super::task::userland_last_address();

    // prints out the error information for this page fault.
    let print_info = || {
        log::error!("");
        log::error!("FS={:#x}", unsafe { io::rdmsr(io::IA32_FS_BASE) },);
        log::error!("GS={:#x}", unsafe { io::rdmsr(io::IA32_GS_BASE) });
        log::error!("");
        log::error!("accessed address: {:#x}", accessed_address);
        log::error!("reason: {:?}", reason);
        log::error!("");
        log::error!("stack: {:#x?}", stack);
    };

    if accessed_address < userland_last_address && scheduler::is_initialized()
        || stack.stack.iret.is_user()
    {
        let signal = scheduler::get_scheduler()
            .current_task()
            .vm
            .handle_page_fault(reason, accessed_address);

        if !signal && stack.stack.iret.is_user() {
            log::error!("Segmentation fault");
            print_info();

            let task = scheduler::get_scheduler().current_task();

            log::error!(
                "process: (tid={}, pid={})",
                task.tid().as_usize(),
                task.pid().as_usize()
            );

            log::error!(
                "process: (path=`{}`)",
                task.path()
                    .expect("userland application does not have a path set")
            );

            task.vm.log();
            task.file_table.log();

            if LOG_PF_PTABLE {
                scheduler::get_scheduler().log_ptable();
            }

            unwind::unwind_stack_trace();

            let task = scheduler::get_scheduler().current_task();
            task.signal(aero_syscall::signal::SIGSEGV);
        } else if !signal {
        } else {
            return;
        }
    }

    unwind::prepare_panic();

    log::error!("Page fault");
    print_info();

    unwind::unwind_stack_trace();

    unsafe {
        loop {
            super::halt();
        }
    }
}
