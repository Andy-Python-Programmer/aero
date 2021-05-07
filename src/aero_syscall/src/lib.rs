#![no_std]
#![feature(asm, decl_macro)]

pub mod consts;
pub mod syscall;

pub use crate::syscall::*;

pub mod prelude {
    pub use crate::consts::*;
    pub use crate::syscall::*;
}

/// Exits the current process with the provided status.
#[inline(always)]
pub fn sys_exit(status: usize) -> usize {
    unsafe { syscall1(60, status) }
}
