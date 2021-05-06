use core::ptr;

use alloc::boxed::Box;

use crate::utils::linker::LinkerSymbol;
use crate::{arch::gdt::PROCESSOR_CONTROL_REGION, utils::io};

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

    // Puts the TLS into kernel's heap and prevents it from being dropped.
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
    let fs_offset = fs_ptr as u64;

    // Set the FS base segment to the fs_offset enabling thread locals and
    // set fs[:0x00] to the fs offset as SystemV abi expects the pointer equal
    // to its offset.
    //
    // SAFTEY: The fs pointer and fs offset are guaranteed to be correct.
    unsafe {
        io::wrmsr(io::IA32_FS_BASE, fs_offset);
        io::wrmsr(io::IA32_GS_BASE, fs_offset);

        *fs_ptr = fs_offset;
    }

    // SAFTEY: Safe to access PROCESSOR_CONTROL_REGION as thread local variables
    // at this point are accessible.
    unsafe {
        PROCESSOR_CONTROL_REGION.fs_offset = fs_offset as usize;
    }
}
