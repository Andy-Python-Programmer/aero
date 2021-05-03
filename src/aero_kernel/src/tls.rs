use crate::arch::memory::alloc::malloc_align;
use crate::utils::linker::LinkerSymbol;

/// Initialize support for thread local.
pub fn init() {
    extern "C" {
        /// The starting byte of the thread data segment.
        static mut __tdata_start: LinkerSymbol;
        /// The ending byte of the thread data segment.
        static mut __tdata_end: LinkerSymbol;
    }

    let size = unsafe { __tdata_end.as_usize() - __tdata_start.as_usize() };
    let tls = malloc_align(size + 8, 8);

    unsafe {
        let tdata_start_ptr: *const u8 = __tdata_start.as_ptr();
        tdata_start_ptr.copy_to(tls, size);
    }
}
