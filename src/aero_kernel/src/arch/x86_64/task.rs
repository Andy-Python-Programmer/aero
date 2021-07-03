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

use alloc::alloc::alloc_zeroed;

use core::alloc::Layout;
use core::ptr::Unique;

use x86_64::VirtAddr;

use super::gdt::TASK_STATE_SEGMENT;
use super::interrupts::IretRegisters;

use crate::mem::AddressSpace;
use crate::utils::stack::StackHelper;

#[repr(C)]
pub(super) struct InterruptFrame {
    pub cr3: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rbx: u64,
    pub rflags: u64,
    pub rip: u64,
}

impl InterruptFrame {
    pub fn new() -> Self {
        Self {
            cr3: 0x00,
            rflags: 0x00,
            r15: 0x00,
            r14: 0x00,
            r13: 0x00,
            r12: 0x00,
            rbp: 0x00,
            rbx: 0x00,
            rip: 0x00,
        }
    }
}

#[repr(C, packed)]
struct KernelTaskFrame {
    pub rdi: usize,
    pub iretq: IretRegisters,
    pub on_finish: usize,
}

pub struct ArchTask {
    context: Unique<InterruptFrame>,
    address_space: Option<AddressSpace>,
    context_switch_rsp: VirtAddr,
}

impl ArchTask {
    pub fn new_idle() -> Self {
        Self {
            context: Unique::dangling(),
            context_switch_rsp: VirtAddr::zero(),
            address_space: None,
        }
    }

    /// Allocates a new kernel task pointing at the provided entry point address. This function
    /// is responsible for creating the kernel task and setting up the context switch stack itself.
    ///
    /// ## Transition
    /// Userland task transition is done through `iretq` method.
    pub fn new_kernel(entry_point: VirtAddr) -> Self {
        extern "C" {
            pub fn iretq_kernelinit();
        }

        let task_stack = unsafe {
            let layout = Layout::from_size_align_unchecked(0x1000, 0x100);
            let raw = alloc_zeroed(layout);

            raw
        };

        let kernel_cr3: u64;

        unsafe {
            asm!("mov {}, cr3", out(reg) kernel_cr3, options(nomem));
        }

        /*
         * Now at this stage, we have mapped the kernek task stack. Now we have to allocate a 16KiB stack
         * for the context switch function on the kernel's heap (which should enough) and create the context
         * switch context itself. This includes the syscall and interrupt contexts.
         */
        let mut context_switch_rsp = unsafe {
            let layout = Layout::from_size_align_unchecked(0x400, 0x100);
            let raw = alloc_zeroed(layout);

            raw as u64 + layout.size() as u64
        };

        let mut context_switch = StackHelper::new(&mut context_switch_rsp);
        let ktask_stack = unsafe { context_switch.offset::<KernelTaskFrame>() };

        ktask_stack.iretq.rsp = task_stack as u64 - 0x08;
        ktask_stack.iretq.rip = entry_point.as_u64();
        ktask_stack.iretq.cs = 0x08; // Ring 0 CS
        ktask_stack.iretq.ss = 0x10; // Ring 0 DS
        ktask_stack.iretq.rflags = 1 << 9; // Interrupts enabled.
        ktask_stack.rdi = 0x00;
        ktask_stack.on_finish = 0xcafebabe;

        let interrupt_stack = unsafe { context_switch.offset::<InterruptFrame>() };
        *interrupt_stack = InterruptFrame::new(); // Sanitize the interrupt stack.

        interrupt_stack.rip = iretq_kernelinit as u64;
        interrupt_stack.cr3 = kernel_cr3;

        let interrupt_stack =
            unsafe { Unique::new_unchecked(interrupt_stack as *mut InterruptFrame) };

        let context_switch_rsp = unsafe { VirtAddr::new_unsafe(context_switch_rsp) };

        Self {
            context: interrupt_stack,
            address_space: None,
            context_switch_rsp,
        }
    }
}

pub fn arch_switch(from: &mut ArchTask, to: &ArchTask) {
    extern "C" {
        fn context_switch(previous: &mut Unique<InterruptFrame>, new: &InterruptFrame);

    }

    unsafe {
        TASK_STATE_SEGMENT.rsp[0] = to.context_switch_rsp.as_u64();

        context_switch(&mut from.context, to.context.as_ref())
    }
}
