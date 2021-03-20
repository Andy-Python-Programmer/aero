use core::{alloc, alloc::GlobalAlloc, ptr::null_mut};

pub struct AeroSystemAllocator;

unsafe impl GlobalAlloc for AeroSystemAllocator {
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

pub fn init_heap() {}
