use core::alloc;

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

use crate::AERO_SYSTEM_ALLOCATOR;

use super::paging::GlobalAllocator;

pub const HEAP_START: usize = 0x444444440000;
pub const HEAP_SIZE: usize = 100 * 1024;

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::Layout) -> ! {
    panic!(
        "Allocation error with size {} and layout {}",
        layout.size(),
        layout.align()
    )
}

/// Initialize the heap.
pub fn init_heap(
    offset_table: &mut OffsetPageTable,
    frame_allocator: &mut GlobalAllocator,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            offset_table
                .map_to(page, frame, flags, frame_allocator)?
                .flush()
        };
    }

    unsafe {
        AERO_SYSTEM_ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
