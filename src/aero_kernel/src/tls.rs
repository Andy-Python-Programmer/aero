use x86_64::VirtAddr;

use crate::arch::memory::alloc::malloc_align;

/// Initialize support for thread local.
pub fn init() {
    extern "C" {
        /// The starting byte of the thread data segment.
        static mut __tdata_start: u8;
        /// The ending byte of the thread data segment.
        static mut __tdata_end: u8;
    }

    let size = unsafe { &__tdata_end as *const u8 as usize - &__tdata_start as *const u8 as usize };

    let tdata_start = VirtAddr::new(unsafe { &__tdata_start } as *const u8 as u64);
    let tls = malloc_align(size + 8, 8);

    unsafe {
        let tdata_start_ptr: *const u8 = tdata_start.as_ptr();
        tdata_start_ptr.copy_to(tls, size);
    }
}
