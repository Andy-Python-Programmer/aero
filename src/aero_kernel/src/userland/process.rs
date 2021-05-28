/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::{
    structures::paging::{mapper::MapToError, *},
    VirtAddr,
};

use xmas_elf::{
    header,
    program::{self, Type},
    ElfFile,
};

use crate::{fs::file_table::FileTable, mem::AddressSpace, prelude::*};
use crate::{mem::paging::FRAME_ALLOCATOR, utils::stack::Stack};

use super::context::Context;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessId(usize);

impl ProcessId {
    #[inline(always)]
    pub(super) const fn new(pid: usize) -> Self {
        Self(pid)
    }

    /// Allocates a new process ID. The caller has to garuntee that
    /// the scheduler is locked until you register the process.
    fn allocate() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

        Self::new(NEXT_PID.fetch_add(1, Ordering::AcqRel))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
}

pub struct Process {
    context: Context,

    pub file_table: FileTable,
    pub process_id: ProcessId,
    pub entry_point: VirtAddr,
    pub state: ProcessState,
}

impl Process {
    /// Allocates a new userland process from the provided executable ELF. This function
    /// is responsible for mapping the loadable program headers, allocating the user stack,
    /// creating the file tables, creating the userland address space which contains the userland
    /// page tables and finally setting up the process context.
    ///
    /// ## Transition
    /// Userland process transition is done through `sysretq` method.
    pub fn from_elf(
        offset_table: &mut OffsetPageTable,
        elf_binary: &ElfFile,
    ) -> Result<Arc<Self>, MapToError<Size4KiB>> {
        let raw_binary = elf_binary.input.as_ptr();

        header::sanity_check(elf_binary).expect("The binary failed the sanity check");

        let address_space = AddressSpace::new()?;

        for header in elf_binary.program_iter() {
            program::sanity_check(header, elf_binary).expect("Failed header sanity check");

            let header_type = header.get_type().expect("Unable to get the header type");
            let header_flags = header.flags();

            if let Type::Load = header_type {
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

                unsafe {
                    memcpy(
                        header.virtual_addr() as *mut u8,
                        raw_binary.add(header.offset() as usize) as *const u8,
                        header.file_size() as usize,
                    );

                    memset(
                        (header.virtual_addr() + header.file_size()) as *mut u8,
                        0,
                        (header.mem_size() - header.file_size()) as usize,
                    );
                }
            }
        }

        /*
         * Allocate and map the user stack for the process.
         */
        let process_stack = {
            let address = unsafe { VirtAddr::new_unsafe(0x80000000) };

            Stack::new_user_pinned(offset_table, address, 0x10000)?
        };

        let stack_top = process_stack.stack_top();
        let entry_point = VirtAddr::new(elf_binary.header.pt2.entry_point());

        let context = {
            let mut context = Context::new();

            context.set_stack_top(stack_top);
            context.set_instruction_ptr(entry_point);
            context.set_page_table(address_space.cr3().start_address());

            context.rflags = 1 << 9; // Interrupts enabled

            context
        };

        let process_id = ProcessId::allocate();
        let file_table = FileTable::new();

        Ok(Arc::new(Self {
            context,
            file_table,
            process_id,
            entry_point,
            state: ProcessState::Running,
        }))
    }

    pub(super) fn get_context_ref(&self) -> &Context {
        &self.context
    }
}
