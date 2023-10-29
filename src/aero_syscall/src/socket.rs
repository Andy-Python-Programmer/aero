#![allow(non_camel_case_types)]

use crate::SocketAddr;

mod c {
    // This should be bindgened.
    pub type socklen_t = u32;

    pub const SCM_RIGHTS: i32 = 1;
    pub const SCM_CREDENTIALS: i32 = 2;

    pub const SOL_SOCKET: i32 = 1;
    pub const SOL_IPV6: i32 = 41;
    pub const SOL_PACKET: i32 = 263;
    pub const SOL_NETLINK: i32 = 270;
}

bitflags::bitflags! {
    // mlibc/abis/mlibc/socket.h
    pub struct MessageFlags: usize {
        /// Indicates that some control data was discarded due to lack of space in the
        /// buffer for ancillary data.
        const CTRUNC = 0x1;
        const DONTROUTE = 0x2;
        const EOR = 0x4;
        const OOB = 0x8;
        /// Requests not to send `SIGPIPE` on errors on stream oriented sockets when the
        /// other end breaks the connection. The `EPIPE` error is still returned.
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
    name_len: c::socklen_t,

    iovec: *mut IoVec, // todo: use Option<NonNull<IoVec>>
    iovec_len: i32,    // todo: use ffi::c_int

    control: *const u8,
    control_len: c::socklen_t,

    pub flags: i32, // todo: use ffi::c_int
}

impl MessageHeader {
    pub fn name_mut<T: SocketAddr>(&mut self) -> Option<&mut T> {
        if self.name.is_null() {
            return None;
        }

        assert!(self.name_len >= core::mem::size_of::<T>() as u32);

        unsafe { Some(&mut *(self.name as *mut T)) }
    }

    pub fn iovecs(&self) -> &[IoVec] {
        unsafe { core::slice::from_raw_parts(self.iovec, self.iovec_len as usize) }
    }

    pub fn iovecs_mut(&mut self) -> &mut [IoVec] {
        unsafe { core::slice::from_raw_parts_mut(self.iovec, self.iovec_len as usize) }
    }

    pub fn control(&self) -> &[ControlMessage] {
        assert!(self.control_len == 0);
        &[]
    }
}

// options/posix/include/bits/posix/iovec.h
#[derive(Debug, Clone)]
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

/// Control Message Header (`struct cmsghdr`).
#[derive(Debug)]
#[repr(C)]
pub struct ControlMessage {
    /// Data byte count, including the header.
    pub cmsg_len: c::socklen_t,
    /// Originating protocol.
    pub cmsg_level: SocketOptionLevel,
    /// Protocol-specific type.
    pub cmsg_type: ControlMessageType,
    // followed by cmsg_data: [u8; cmsg_len - sizeof(struct cmsghdr)]
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(i32)]
pub enum ControlMessageType {
    Rights = c::SCM_RIGHTS,
    Credentials = c::SCM_CREDENTIALS,
}

#[derive(Debug, Copy, Clone, PartialEq, FromPrimitive)]
#[repr(i32)]
pub enum SocketOptionLevel {
    Socket = c::SOL_SOCKET,
    Ipv6 = c::SOL_IPV6,
    Packet = c::SOL_PACKET,
    Netlink = c::SOL_NETLINK,
}
