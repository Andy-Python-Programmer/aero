use crate::SocketAddr;

// sysdeps/aero/include/abi-bits/socket.h
#[derive(Debug)]
#[repr(C)]
pub struct MessageHeader {
    /// Pointer to the socket address structure.
    name: *mut u8,
    /// Size of the socket address structure.
    name_len: usize,

    iovec: *mut IoVec, // todo: use Option<NonNull<IoVec>>
    iovec_len: i32,    // todo: use ffi::c_int

    control: *const u8,
    control_len: usize,

    flags: i32, // todo: use ffi::c_int
}

impl MessageHeader {
    pub fn name_mut<T: SocketAddr>(&mut self) -> Option<&mut T> {
        if self.name.is_null() {
            return None;
        }

        assert!(self.name_len == core::mem::size_of::<T>());

        // SAFETY: We know that the `name` pointer is valid and we have an exclusive reference to it.
        // The size of name is checked above with the size of `T` and `T` is a `SocketAddr` so, its
        // safe to create a mutable reference of `T` from the ptr.
        unsafe { Some(&mut *(self.name as *mut T)) }
    }

    pub fn iovecs(&self) -> &[IoVec] {
        // SAFETY: We know that the `iovec` pointer is valid, initialized.
        unsafe { core::slice::from_raw_parts(self.iovec, self.iovec_len as usize) }
    }

    pub fn iovecs_mut(&mut self) -> &mut [IoVec] {
        // SAFETY: We know that the `iovec` pointer is valid, initialized and we have
        // exclusive access so, its safe to construct a mutable slice from it.
        unsafe { core::slice::from_raw_parts_mut(self.iovec, self.iovec_len as usize) }
    }
}

// options/posix/include/bits/posix/iovec.h
#[derive(Debug)]
#[repr(C)]
pub struct IoVec {
    base: *mut u8, // todo: use Option<NonNull<u8>>
    len: usize,
}

impl IoVec {
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: We know that the `base` pointer is valid, initialized and we have
        // exclusive access so, its safe to construct a mutable slice from it.
        unsafe { core::slice::from_raw_parts_mut(self.base, self.len) }
    }

    /// Returns the length of the I/O vector.
    pub fn len(&self) -> usize {
        self.len
    }
}
