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

use crate::mem::paging::*;
use crate::prelude::*;

use super::gdt::{Ring, TASK_STATE_SEGMENT};

use crate::mem::AddressSpace;
use crate::utils::stack::Stack;

#[repr(C)]
#[derive(Default)]
struct Context {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    isr_number: u64,
    err_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

pub struct ArchTask {
    context: Context,
    address_space: AddressSpace,
    context_switch_rsp: VirtAddr,

    rpl: Ring,
}

impl ArchTask {
    pub fn new_idle() -> Self {
        Self {
            context: Context::default(),
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
        let task_stack = unsafe {
            // We want the task stack to be page aligned.
            let layout = Layout::from_size_align_unchecked(0x1000, 0x100);
            let raw = alloc_zeroed(layout);

            raw
        };

        // Get the current active address space as we are making the task for
        // the kernel itself.
        let address_space = AddressSpace::this();

        /*
         * Now at this stage, we have mapped the kernel task stack. Now we have to set up the
         * context for the kernel task required for the context switch.
         */
        let context = Context {
            r15: 0x00,
            r14: 0x00,
            r13: 0x00,
            r12: 0x00,
            r11: 0x00,
            r10: 0x00,
            r9: 0x00,
            r8: 0x00,
            rbp: 0x00,
            rdi: 0x00,
            rsi: 0x00,
            rdx: 0x00,
            rcx: 0x00,
            rbx: 0x00,
            rax: 0x00,
            isr_number: 0x00,
            err_code: 0x00,

            rip: entry_point.as_u64(), // Set the instruction pointer to the entry point.
            cs: 0x08,                  // Kernel code segment.
            rflags: 0x202,             // Interrupts enabled.
            rsp: task_stack as u64,    // Set the stack pointer to the task stack.
            ss: 0x10,                  // Kernel stack segment.
        };

        Self {
            context,
            address_space,
            context_switch_rsp: VirtAddr::new(task_stack as u64),

            // Since we are creating a kernel task, we set the ring privilage
            // level to ring 0.
            rpl: Ring::Ring0,
        }
    }

    pub fn exec(&mut self, executable: &ElfFile) -> Result<(), MapToError<Size4KiB>> {
        header::sanity_check(executable).expect("Failed sanity check"); // Sanity check the provided ELF executable

        let raw_executable = executable.input.as_ptr();

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

                let mut addr = None;

                for page in page_range {
                    let frame = unsafe {
                        FRAME_ALLOCATOR
                            .allocate_frame()
                            .ok_or(MapToError::FrameAllocationFailed)?
                    };

                    if addr.is_none() {
                        addr = Some(frame.start_address().as_u64());
                    }

                    unsafe { offset_table.map_to(page, frame, flags, &mut FRAME_ALLOCATOR) }?
                        .flush();
                }

                let addr = addr.unwrap();

                // Segments need to be cleared to zero.
                unsafe {
                    let buffer: *mut u8 = (crate::PHYSICAL_MEMORY_OFFSET + addr).as_mut_ptr();

                    memcpy(
                        buffer,
                        raw_executable.add(header.offset() as usize),
                        header.file_size() as usize,
                    );
                }
            }
        }

        let task_stack = {
            // 2 GiB is chosen arbitrarily (as programs are expected to fit below 2 GiB).
            let address = unsafe { VirtAddr::new_unsafe(0x8000_0000_0000) };

            // Allocate the user stack (it's page aligned).
            Stack::new_user_pinned(&mut offset_table, address, 0x64000)?
        };

        unsafe {
            // Set the new stack pointer in TSS.
            TASK_STATE_SEGMENT.rsp[0] = task_stack.stack_top().as_u64();
        }

        address_space.switch(); // Perform the address space switch
        self.address_space = address_space; // Update the address space reference

        extern "C" {
            // fn jump_userland_exec(stack: VirtAddr, rip: VirtAddr, rflags: u64);
        }

        // let entry_point = VirtAddr::new(executable.header.pt2.entry_point());

        // unsafe { jump_userland_exec(task_stack.stack_top(), entry_point, 1 << 9) }

        Ok(())
    }
}

/// This function is responsible for performing the inner task switch. Firstly it sets the
/// new RSP in the TSS and then performes the actual context switch (saving the previous tasks
/// state in its context and then switching to the new task).
pub fn arch_switch(to: &mut ArchTask) {
    extern "C" {
        fn task_spinup(context: u64, cr3: u64);
    }

    unsafe {
        TASK_STATE_SEGMENT.rsp[0] = to.context_switch_rsp.as_u64(); // Set the stack pointer in the TSS.
        task_spinup((&mut to.context as *mut Context) as u64, 0x00) // Perform the context switch.
    }
}
