use core::mem;

pub mod alloc;
pub mod paging;

const WORD_SIZE: usize = mem::size_of::<usize>();

/// Memset
///
/// Fill a block of memory with a specified value.
///
/// This faster implementation works by setting bytes not one-by-one, but in groups of 8 bytes
/// (or 4 bytes in the case of 32-bit architectures).
#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    let c = mem::transmute([c as u8; WORD_SIZE]);
    let n_usize = n / WORD_SIZE;

    let mut i = 0;

    // Set `WORD_SIZE` bytes at a time
    let n_fast = n_usize * WORD_SIZE;

    while i < n_fast {
        *((dest as usize + i) as *mut usize) = c;

        i += WORD_SIZE;
    }

    let c = c as u8;

    // Set 1 byte at a time
    while i < n {
        *((dest as usize + i) as *mut u8) = c;
        i += 1;
    }

    dest
}
