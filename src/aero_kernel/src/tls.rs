//! Thread Local Storage (TLS) are per-thread global variables. On 64-bit each CPU core's
//! `gs` GDT segment points to the thread local memory area where the thread local static's
//! live. TLS statics are simply accessed through an offset from `gs`.
//!
//! ## Notes
//! * <https://wiki.osdev.org/Thread_Local_Storage>
//! * <https://doc.rust-lang.org/std/thread/struct.LocalKey.html>

use core::alloc::Layout;
use core::mem;
use core::ptr;

use alloc::alloc::alloc_zeroed;

use spin::Once;

use crate::utils::io;
use crate::utils::linker::LinkerSymbol;

use crate::arch::gdt::PROCESSOR_CONTROL_REGION;

static THREAD_LOCAL_STORAGE: Once<ThreadLocalStorage> = Once::new();

/// The TCB (Thread Control Block) containg the self pointer to itself and that means
/// the size of TCB will **always** be the size of a pointer.
#[repr(C)]
struct ThreadControlBlock {
    self_pointer: *mut Self,
}

#[repr(C)]
struct ThreadLocalStorage {
    /// Pointer to the allocated memory for the TLS.
    pointer: usize,
    tcb_offset: usize,
}

impl ThreadLocalStorage {
    #[inline]
    fn new(pointer: usize, tcb_offset: usize) -> Self {
        Self {
            pointer,
            tcb_offset,
        }
    }
}

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

    // Here we add the size of TCB to the total size to store the TCB pointer.
    let total_tls_size = total_size + mem::size_of::<ThreadControlBlock>();
    let tls_layout = unsafe {
        Layout::from_size_align_unchecked(total_tls_size, mem::align_of::<ThreadControlBlock>())
    };

    let tls_raw_ptr = unsafe { alloc_zeroed(tls_layout) };
    let tls_offset = tls_raw_ptr as usize;

    unsafe {
        ptr::copy(__tdata_start.as_ptr(), tls_raw_ptr, tdata_size);
        ptr::write_bytes(
            (tls_offset + tdata_size) as *mut u8,
            0,
            total_tls_size - tdata_size,
        );
    }

    let tcb_ptr = ((tls_raw_ptr as u64) + (total_size as u64)) as *mut u64;
    let tcb_offset = tcb_ptr as usize;

    /* Set the FS base segment to the tcb_offset to enable thread locals and
     * set fs[:0x00] to the tcb offset as SystemV abi expects the pointer equal
     * to its offset.
     *
     * SAFTEY: The fs pointer and tcb offset are guaranteed to be correct.
     */
    unsafe {
        io::wrmsr(io::IA32_FS_BASE, tcb_offset as u64);

        *tcb_ptr = tcb_offset as u64;
    }

    THREAD_LOCAL_STORAGE.call_once(move || ThreadLocalStorage::new(tls_offset, tcb_offset));

    // SAFTEY: Safe to access thread local variables as at this point are accessible.
    unsafe {
        PROCESSOR_CONTROL_REGION.fs_offset = tcb_offset as usize;
    }
}
