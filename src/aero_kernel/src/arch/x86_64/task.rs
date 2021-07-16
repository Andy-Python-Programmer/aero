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
use xmas_elf::program::Type;
use xmas_elf::{header, program, ElfFile};

use core::alloc::Layout;
use core::mem;
use core::ptr::Unique;

use crate::mem::paging::*;

use super::gdt::{Ring, TASK_STATE_SEGMENT};
use super::interrupts::IretRegisters;

use crate::mem::AddressSpace;
use crate::utils::stack::{Stack, StackHelper};

#[repr(C)]
struct InterruptFrame {
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
    address_space: AddressSpace,
    context_switch_rsp: VirtAddr,

    rpl: Ring,
}

impl ArchTask {
    pub fn new_idle() -> Self {
        Self {
            context: Unique::dangling(),
            context_switch_rsp: VirtAddr::zero(),

            // Since the IDLE task is a special kernel task, we use the kernel's
            // address space here and we also use the kernel privilage level here.
            address_space: AddressSpace::this(),
            rpl: Ring::Ring0,
        }
    }

    /// Allocates a new kernel task pointing at the provided entry point address. This function
    /// is responsible for creating the kernel task and setting up the context switch stack itself.
    pub fn new_kernel(entry_point: VirtAddr) -> Self {
        extern "C" {
            pub fn iretq_kernelinit();
        }

        let task_stack = unsafe {
            // We want the task stack to be page aligned.
            let layout = Layout::from_size_align_unchecked(0x1000, 0x100);
            let raw = alloc_zeroed(layout);

            raw
        };

        // Get the current active address space as we are making the task for the kernel itself.
        let address_space = AddressSpace::this();

        /*
         * Now at this stage, we have mapped the kernel task stack. Now we have to allocate memory
         * for the context switch function on the kernel's heap and create the context switch context
         * itself. This includes the syscall and interrupt contexts.
         */
        let mut context_switch_rsp = unsafe {
            // Size needed for the context switch is simply the size of task frame added to the size
            // of interrupt frame.
            let size = mem::size_of::<KernelTaskFrame>() + mem::size_of::<InterruptFrame>();

            // We want the memory that we allocate for context switch to be page aligned.
            let layout = Layout::from_size_align_unchecked(size, 0x100);
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
        interrupt_stack.cr3 = address_space.cr3().start_address().as_u64();

        let interrupt_stack =
            unsafe { Unique::new_unchecked(interrupt_stack as *mut InterruptFrame) };

        let context_switch_rsp = unsafe { VirtAddr::new_unsafe(context_switch_rsp) };

        Self {
            context: interrupt_stack,
            address_space,
            context_switch_rsp,

            // Since we are creating a kernel task, we set the ring privilage
            // level to ring 0.
            rpl: Ring::Ring0,
        }
    }

    pub fn exec(&mut self, executable: &ElfFile) -> Result<(), MapToError<Size4KiB>> {
        header::sanity_check(executable).expect("Failed sanity check"); // Sanity check the provided ELF executable

        let mut address_space = if self.rpl == Ring::Ring0 {
            // If the kernel task wants to execute an executable, then we have to
            // create a new address space for it as we cannot use the kernel's address space
            // here.
            AddressSpace::new()?
        } else {
            // If we are the user who wants to execute an executable, we can just use the
            // current address space allocated for the user and deallocate all of the user
            // page entries.
            //
            // TODO: deallocate the user address space's page entries.
            AddressSpace::this()
        };

        // Get the page map reference from the address space.
        let mut offset_table = address_space.offset_page_table();

        for header in executable.program_iter() {
            program::sanity_check(header, executable).expect("Failed header sanity check");

            let header_type = header
                .get_type()
                .expect("Failed to get program header type");

            let header_flags = header.flags();

            if header_type == Type::Load {
                let page_range = {
                    let start_addr = VirtAddr::new(header.virtual_addr());
                    let end_addr = start_addr + header.mem_size() - 1u64;

                    let start_page: Page = Page::containing_address(start_addr);
                    let end_page = Page::containing_address(end_addr);

                    Page::range_inclusive(start_page, end_page)
                };

                let mut flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE;

                if !header_flags.is_execute() {
                    flags |= PageTableFlags::NO_EXECUTE;
                }

                for page in page_range {
                    let frame = unsafe {
                        FRAME_ALLOCATOR
                            .allocate_frame()
                            .ok_or(MapToError::FrameAllocationFailed)?
                    };

                    unsafe { offset_table.map_to(page, frame, flags, &mut FRAME_ALLOCATOR) }?
                        .flush();
                }
            }
        }

        let task_stack = {
            // 2 GiB is chosen arbitrarily (as programs are expected to fit below 2 GiB).
            let address = unsafe { VirtAddr::new_unsafe(0x8000_0000_0000) };

            // Allocate the user stack (it's page aligned).
            Stack::new_user_pinned(&mut offset_table, address, 0x64000)?
        };

        self.context = Unique::dangling();

        unsafe {
            // Set the new stack pointer in TSS.
            TASK_STATE_SEGMENT.rsp[0] = task_stack.stack_top().as_u64();
        }

        address_space.switch(); // Perform the address space switch
        self.address_space = address_space; // Update the address space reference

        extern "C" {
            fn jump_userland_exec(stack: VirtAddr, rip: VirtAddr, rflags: u64);
        }

        let entry_point = VirtAddr::new(executable.header.pt2.entry_point());

        // unsafe { jump_userland_exec(task_stack.stack_top(), entry_point, 1 << 9) }

        Ok(())
    }
}

/// This function is responsible for performing the inner task switch. Firstly it sets the
/// new RSP in the TSS and then performes the actual context switch (saving the previous tasks
/// state in its context and then switching to the new task).
pub fn arch_switch(from: &mut ArchTask, to: &ArchTask) {
    extern "C" {
        fn context_switch(previous: &mut Unique<InterruptFrame>, new: &InterruptFrame);
    }

    unsafe {
        TASK_STATE_SEGMENT.rsp[0] = to.context_switch_rsp.as_u64(); // Set the stack pointer
        context_switch(&mut from.context, to.context.as_ref()) // Perform the switch
    }
}
