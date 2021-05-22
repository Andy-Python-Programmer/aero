use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::{structures::paging::*, VirtAddr};

use xmas_elf::{
    header,
    program::{self, Type},
    ElfFile,
};

use crate::mem::paging::FRAME_ALLOCATOR;
use crate::prelude::*;

/// The process id counter. Increment after a new process is created.
static PID_COUNTER: PIDCounter = PIDCounter::new();

#[derive(Debug)]
#[repr(transparent)]
struct PIDCounter(AtomicUsize);

impl PIDCounter {
    /// Create a new process id counter.
    #[inline(always)]
    const fn new() -> Self {
        Self(AtomicUsize::new(1))
    }

    /// Increment the process id by 1.
    #[inline(always)]
    fn next(&self) -> usize {
        self.0.fetch_add(1, Ordering::AcqRel)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
}

#[derive(Debug)]
pub struct Process {
    pub pid: usize,
    pub entry_point: VirtAddr,
    pub state: ProcessState,
}

impl Process {
    /// Create a new process from the provided [ElfFile].
    pub fn from_elf(offset_table: &mut OffsetPageTable, binary: &ElfFile) -> Self {
        header::sanity_check(binary).expect("The binary failed the sanity check");

        for header in binary.program_iter() {
            program::sanity_check(header, binary).expect("Failed header sanity check");

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

                let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

                if !header_flags.is_execute() {
                    flags |= PageTableFlags::NO_EXECUTE;
                }

                if header_flags.is_write() {
                    flags |= PageTableFlags::WRITABLE;
                }

                for page in page_range {
                    let frame = unsafe { FRAME_ALLOCATOR.allocate_frame().unwrap() };

                    unsafe { offset_table.map_to(page, frame, flags, &mut FRAME_ALLOCATOR) }
                        .unwrap()
                        .flush();
                }

                unsafe {
                    // memcpy(
                    //     header.virtual_addr() as *mut u8,
                    //     binary.input.as_ptr().add(header.offset() as usize) as *const u8,
                    //     header.file_size() as usize,
                    // );

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
         *
         * 0x7FFFFFFFF000 + Size4KiB = 0x800000000000 is where we want the
         * stack to be.
         */
        {
            let page: Page = Page::containing_address(VirtAddr::new(0x7FFFFFFFF000));
            let frame = unsafe { FRAME_ALLOCATOR.allocate_frame().unwrap() };

            unsafe {
                offset_table.map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT
                        | PageTableFlags::USER_ACCESSIBLE
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::NO_EXECUTE,
                    &mut FRAME_ALLOCATOR,
                )
            }
            .unwrap()
            .flush();
        }

        let entry_point = VirtAddr::new(binary.header.pt2.entry_point());

        // unsafe {
        // super::jump_userland(entry_point.as_u64() as _);
        // }

        Self {
            pid: PID_COUNTER.next(),
            entry_point,
            state: ProcessState::Running,
        }
    }

    /// Create a new process from a function.
    pub fn from_function(function: unsafe extern "C" fn()) -> Self {
        let this = Self {
            pid: PID_COUNTER.next(),
            entry_point: VirtAddr::new((&function as *const _) as u64),
            state: ProcessState::Running,
        };

        this
    }
}
