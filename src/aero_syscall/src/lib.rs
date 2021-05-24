#![no_std]
#![feature(asm, decl_macro)]

pub mod consts;
pub mod syscall;

pub use crate::syscall::*;

pub mod prelude {
    pub use crate::consts::*;
    pub use crate::syscall::*;

    pub use crate::{AeroSyscallError, AeroSyscallResult};
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(isize)]
pub enum AeroSyscallError {
    Unknown = isize::MAX,
}

pub type AeroSyscallResult = Result<usize, AeroSyscallError>;

pub fn syscall_result_as_usize(result: AeroSyscallResult) -> usize {
    match result {
        Ok(value) => value as _,
        Err(error) => -(error as isize) as _,
    }
}

/// Exits the current process with the provided status.
#[inline(always)]
pub fn sys_exit(status: usize) -> usize {
    unsafe { syscall1(prelude::SYS_EXIT, status) }
}

#[inline(always)]
pub fn sys_write(fd: usize, buf: &[u8]) -> usize {
    unsafe {
        syscall3(
            prelude::SYS_WRITE,
            fd as usize,
            buf.as_ptr() as usize,
            buf.len(),
        )
    }
}
