use core::ptr;

use alloc::boxed::Box;

use crate::utils::io;
use crate::utils::linker::LinkerSymbol;

/// Initialize support for the `#[thread_local]` attribute.
pub fn init() {
    extern "C" {
        /// The starting byte of the thread data segment.
        static __tdata_start: LinkerSymbol;
        /// The ending byte of the thread data segment.
        static __tdata_end: LinkerSymbol;
        /// The starting byte of the thread BSS segment.
        static __tbss_start: LinkerSymbol;
        /// The ending byte of the thread BSS segment.
        static __tbss_end: LinkerSymbol;
    }

    // SAFTEY: These linker symbols are **guaranteed** to be present.
    let total_size = unsafe { __tbss_end.as_usize() - __tdata_start.as_usize() };
    let tdata_size = unsafe { __tdata_end.as_usize() - __tdata_start.as_usize() };

    // Here we add 8 to the total size to store the TCB pointer.
    let total_tls_size = total_size + 8;
    let mut tls_raw_ptr = Box::<[u8]>::new_uninit_slice(total_tls_size);

    unsafe {
        ptr::copy(
            __tdata_start.as_ptr(),
            tls_raw_ptr.as_mut_ptr() as *mut u8,
            tdata_size,
        );

        ptr::write_bytes(
            ((tls_raw_ptr.as_mut_ptr() as usize) + tdata_size) as *mut u8,
            0,
            total_tls_size - tdata_size,
        );
    }

    let tls_ptr = unsafe { Box::into_raw(tls_raw_ptr.assume_init()) };
    let fs_ptr = ((tls_ptr as *const u8 as u64) + (total_size as u64)) as *mut u64;

    unsafe {
        io::wrmsr(io::IA32_FS_BASE, fs_ptr as u64);

        // The SystemV abi expects fs[:0x00] to be the address of fs.
        *fs_ptr = fs_ptr as u64;
    }
}
