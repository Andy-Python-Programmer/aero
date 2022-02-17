/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
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

use crate::syscall::{RegistersFrame, SyscallFrame};
use crate::userland;
use crate::userland::scheduler;
use crate::utils::StackHelper;

use super::interrupts::InterruptStack;

const REDZONE_SIZE: u64 = 128;
const SYSCALL_INSTRUCTION_SIZE: u64 = 2;

#[repr(C)]
#[derive(Debug)]
pub struct SignalFrame {
    restart_syscall: u64,
    frame: InterruptStack,
    sigmask: u64,
}

impl SignalFrame {
    fn from_interrupt(frame: &mut InterruptStack, sigmask: u64) -> SignalFrame {
        SignalFrame {
            restart_syscall: u64::MAX,
            frame: *frame,
            sigmask,
        }
    }
}

pub fn interrupt_check_signals(stack: &mut InterruptStack) {
    // SAFTEY: If this interrupt did not originate from userland then we cannot
    // check for signals since the scheduler might not be initialized.
    if !stack.iret.is_user() {
        return;
    }

    if let Some((signal, entry)) = userland::signals::check_for_signals() {
        if let aero_syscall::signal::SignalHandler::Handle(func) = entry.handler() {
            let task = scheduler::get_scheduler().current_task();

            let signals = task.signals();
            let old_mask = signals.blocked_mask();

            let signal_frame = SignalFrame::from_interrupt(stack, old_mask);

            signals.set_mask(
                aero_syscall::signal::SigProcMask::Block,
                1u64 << signal,
                None,
            );

            // We cannot straight away update the stack pointer from the stack
            // helper, since it will created a reference to a packed field which
            // is undefined behavior. So we create a copy of the current rsp and
            // update the actual rsp with the updated rsp.
            let mut ptr = stack.iret.rsp;
            let mut writer = StackHelper::new(&mut ptr);

            // Signal handlers are executed on the same stack, but 128 bytes
            // known as the red zone is subtracted from the stack before
            // anything is pushed to the stack. This allows small leaf
            // functions to use 128 bytes of stack space without reserving
            // stack space by subtracting from the stack pointer.
            writer.skip_by(REDZONE_SIZE);

            unsafe {
                writer.write(signal_frame);
                writer.write(entry.sigreturn());
            }

            stack.iret.rsp = ptr;
            stack.iret.rip = func as u64;
            stack.scratch.rdi = signal as u64;
        }
    }
}

/// Helper function to check for any pending signals from a sycall.
pub fn syscall_check_signals(
    _syscall_result: isize,
    _syscall: &mut SyscallFrame,
    _registers: &mut RegistersFrame,
) {
    if let Some((_signal, entry)) = userland::signals::check_for_signals() {
        if let aero_syscall::signal::SignalHandler::Handle(_) = entry.handler() {
            todo!()
        }
    }
}

pub fn sigreturn(sys: &mut SyscallFrame, regs: &mut RegistersFrame) -> usize {
    let mut writer = StackHelper::new(&mut sys.rsp);
    let signal_frame = unsafe { writer.get::<SignalFrame>() };

    let current_task = scheduler::get_scheduler().current_task();

    current_task.signals().set_mask(
        aero_syscall::signal::SigProcMask::Set,
        signal_frame.sigmask,
        None,
    );

    writer.get_by(REDZONE_SIZE);

    let result = signal_frame.frame.scratch.rax;

    let ret_regs = RegistersFrame {
        cr2: 0, // TODO: we have to fill up the cr2 as well
        rax: signal_frame.frame.scratch.rax,
        rbx: signal_frame.frame.preserved.rbx,
        rcx: signal_frame.frame.scratch.rcx,
        rdx: signal_frame.frame.scratch.rdx,
        rsi: signal_frame.frame.scratch.rsi,
        rdi: signal_frame.frame.scratch.rdi,
        rbp: signal_frame.frame.preserved.rbp,
        r8: signal_frame.frame.scratch.r8,
        r9: signal_frame.frame.scratch.r9,
        r10: signal_frame.frame.scratch.r10,
        r11: signal_frame.frame.scratch.r11,
        r12: signal_frame.frame.preserved.r12,
        r13: signal_frame.frame.preserved.r13,
        r14: signal_frame.frame.preserved.r14,
        r15: signal_frame.frame.preserved.r15,
    };

    sys.rflags = signal_frame.frame.iret.rflags;
    sys.rip = signal_frame.frame.iret.rip;

    if signal_frame.restart_syscall != u64::MAX {
        sys.rip -= SYSCALL_INSTRUCTION_SIZE;
    }

    *regs = ret_regs;
    result as usize
}
