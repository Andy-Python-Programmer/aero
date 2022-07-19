// sysdeps/aero/include/abi-bits/socket.h
#[derive(Debug)]
#[repr(C)]
pub struct MessageHeader {
    name: *const u8,
    name_len: usize,

    iovec: *mut IoVec, // todo: use Option<NonNull<IoVec>>
    iovec_len: i32,    // todo: use ffi::c_int

    control: *const u8,
    control_len: usize,

    flags: i32, // todo: use ffi::c_int
}

// options/posix/include/bits/posix/iovec.h
#[derive(Debug)]
#[repr(C)]
pub struct IoVec {
    base: *mut u8,
    len: usize,
}
