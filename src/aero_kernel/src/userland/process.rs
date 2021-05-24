use alloc::sync::Arc;
use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::{structures::paging::*, VirtAddr};

use xmas_elf::{
    header,
    program::{self, Type},
    ElfFile,
};

use crate::prelude::*;
use crate::{mem::paging::FRAME_ALLOCATOR, utils::stack::Stack};

use super::context::Context;

static PID_COUNTER: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ProcessId(usize);

impl ProcessId {
    #[inline(always)]
    const fn new(pid: usize) -> Self {
        Self(pid)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
}

pub struct Process {
    context: Context,

    pub process_id: ProcessId,
    pub entry_point: VirtAddr,
    pub state: ProcessState,
}

impl Process {
    /// Create a new process from the provided [ElfFile].
    pub fn from_elf(offset_table: &mut OffsetPageTable, elf_binary: &ElfFile) -> Arc<Self> {
        let raw_binary = elf_binary.input.as_ptr();

        header::sanity_check(elf_binary).expect("The binary failed the sanity check");

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
                    let frame = unsafe { FRAME_ALLOCATOR.allocate_frame().unwrap() };

                    unsafe { offset_table.map_to(page, frame, flags, &mut FRAME_ALLOCATOR) }
                        .unwrap()
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

            Stack::allocate_user(offset_table, address, 0x10000).expect(
                "Failed to allocate stack for the user process (size=0x10000, address=0x80000000)",
            )
        };

        let stack_top = process_stack.stack_top();
        let entry_point = VirtAddr::new(elf_binary.header.pt2.entry_point());

        let mut context = Context::new();

        context.set_stack_top(stack_top);
        context.set_instruction_ptr(entry_point);
        context.rflags = 0x2000;

        let process_id = ProcessId::new(PID_COUNTER.fetch_add(1, Ordering::AcqRel));

        let this = Self {
            context,
            process_id,
            entry_point,
            state: ProcessState::Running,
        };

        Arc::new(this)
    }

    pub(super) fn get_context_ref(&self) -> &Context {
        &self.context
    }
}
