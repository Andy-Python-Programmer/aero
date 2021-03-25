use core::{alloc, alloc::GlobalAlloc, ptr::null_mut};

use x86_64::{
    structures::paging::{
        mapper::MapToError, page::PageRangeInclusive, FrameAllocator, Mapper, OffsetPageTable,
        Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

use crate::utils::memory::Locked;

pub const HEAP_START: usize = 0x444444440000;
pub const HEAP_SIZE: usize = 100 * 1024;

pub struct AeroSystemAllocator;

impl AeroSystemAllocator {
    pub const fn new() -> Self {
        Self {}
    }
}

unsafe impl GlobalAlloc for Locked<AeroSystemAllocator> {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: alloc::Layout) {
        panic!(
            "Requested to dealloc *{:?} with size {} and alignment {}",
            ptr,
            layout.size(),
            layout.align()
        )
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::Layout) -> ! {
    panic!(
        "Allocation error with size {} and layout {}",
        layout.size(),
        layout.align()
    )
}

fn get_page_range() -> PageRangeInclusive {
    let heap_start = VirtAddr::new(HEAP_START as u64);
    let heap_end = heap_start + HEAP_SIZE - 1u64;
    let heap_start_page = Page::containing_address(heap_start);
    let heap_end_page = Page::containing_address(heap_end);

    Page::range_inclusive(heap_start_page, heap_end_page)
}

/// Initialize the heap.
pub fn init_heap(
    offset_table: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = get_page_range();

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            offset_table
                .map_to(page, frame, flags, frame_allocator)?
                .flush();
        }
    }

    Ok(())
}
