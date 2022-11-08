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

use aero_syscall::signal::{SigProcMask, SignalFlags};
use aero_syscall::SyscallError;

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

    fn from_syscall(
        restart: bool,
        syscall_result: u64,
        frame: &mut InterruptStack,
        sigmask: u64,
    ) -> SignalFrame {
        SignalFrame {
            restart_syscall: if restart {
                frame.scratch.rax // syscall number
            } else {
                syscall_result
            },
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
            signals.set_mask(SigProcMask::Block, Some(1u64 << signal), None);

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

pub fn syscall_check_signals(syscall_result: isize, stack: &mut InterruptStack) {
    if let Some((signal, entry)) = userland::signals::check_for_signals() {
        if let aero_syscall::signal::SignalHandler::Handle(func) = entry.handler() {
            let task = scheduler::get_scheduler().current_task();

            let signals = task.signals();
            let old_mask = signals.blocked_mask();

            let syscall_rresult = aero_syscall::isize_as_syscall_result(syscall_result);
            let restart_syscall = syscall_rresult == Err(SyscallError::EINTR)
                && entry.flags().contains(SignalFlags::SA_RESTART);

            #[cfg(feature = "syslog")]
            log::warn!("syscall routine signaled: (restart={restart_syscall})");

            let signal_frame =
                SignalFrame::from_syscall(restart_syscall, syscall_result as _, stack, old_mask);
            signals.set_mask(SigProcMask::Block, Some(1u64 << signal), None);

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

pub fn sigreturn(stack: &mut InterruptStack) -> usize {
    let mut writer = StackHelper::new(&mut stack.iret.rsp);
    let signal_frame = unsafe { writer.get::<SignalFrame>() };

    let current_task = scheduler::get_scheduler().current_task();

    current_task.signals().set_mask(
        aero_syscall::signal::SigProcMask::Set,
        Some(signal_frame.sigmask),
        None,
    );

    writer.get_by(REDZONE_SIZE);

    let result = signal_frame.frame.scratch.rax;
    *stack = signal_frame.frame;

    if signal_frame.restart_syscall != u64::MAX {
        stack.iret.rip -= SYSCALL_INSTRUCTION_SIZE;
    }

    result as usize
}
