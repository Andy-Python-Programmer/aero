use crate::SocketAddr;

bitflags::bitflags! {
    // mlibc/abis/mlibc/socket.h
    pub struct MessageFlags: usize {
        const CTRUNC = 0x1;
        const DONTROUTE = 0x2;
        const EOR = 0x4;
        const OOB = 0x8;
        const NOSIGNAL = 0x10;
        const PEEK = 0x20;
        const TRUNC = 0x40;
        const WAITALL = 0x80;
        const FIN = 0x200;
        const CONFIRM = 0x800;

        // Linux extensions.
        const DONTWAIT = 0x1000;
        const CMSG_CLOEXEC = 0x2000;
        const MORE = 0x4000;
        const FASTOPEN = 0x20000000;
    }
}

// mlibc/abis/mlibc/socket.h
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

        // SAFETY: We know that the `name` pointer is valid and we have an exclusive reference to
        // it. The size of name is checked above with the size of `T` and `T` is a `SocketAddr` so,
        // its safe to create a mutable reference of `T` from the ptr.
        unsafe { Some(&mut *(self.name as *mut T)) }
    }

    pub fn iovecs(&self) -> &[IoVec] {
        // SAFETY: We know that the `iovec` pointer is valid, initialized.
        unsafe { core::slice::from_raw_parts(self.iovec, self.iovec_len as usize) }
    }

    pub fn iovecs_mut(&mut self) -> &mut [IoVec] {
        // SAFETY: We know that the `iovec` pointer is valid, initialized and we have exclusive
        // access so, its safe to construct a mutable slice from it.
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
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: We know that the `base` pointer is valid and initialized.
        unsafe { core::slice::from_raw_parts_mut(self.base, self.len) }
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        // SAFETY: We know that the `base` pointer is valid, initialized and we have exclusive
        // access so, its safe to construct a mutable slice from it.
        unsafe { core::slice::from_raw_parts_mut(self.base, self.len) }
    }

    /// Returns the length of the I/O vector.
    pub fn len(&self) -> usize {
        self.len
    }
}
