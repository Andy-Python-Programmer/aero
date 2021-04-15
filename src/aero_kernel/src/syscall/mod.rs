//! System Calls are used to call a kernel service from user land.
//!
//! | %rax   | Name                    |
//! |--------|-------------------------|
//! | 0      | read                    |
//! | 1      | write                   |
//! | 2      | open                    |
//! | 3      | close                   |
//!
//! **Notes**: <https://wiki.osdev.org/System_Calls>

pub mod fs;
pub mod time;

pub use fs::*;
pub use time::*;

use crate::arch::cpu::CPUState;
use crate::arch::interrupts::InterruptStackFrame;
use crate::{interrupt, println};

pub enum SyscallError {
    /// Operation not permitted.
    NotPermitted,
    /// No such file or directory.
    NoEntry,
    /// Invalid argument.
    InvalidValue,
    /// Syscall not implemented.
    NoCall,
}

pub type SyscallResult<T> = Result<T, SyscallError>;

interrupt!(syscall, unsafe {
    let cpu_state = CPUState::new();

    match cpu_state.ax {
        0 => unimplemented!(),
        1 => unimplemented!(),
        2 => unimplemented!(),
        3 => unimplemented!(),
        _ => println!("Invalid syscall"),
    }
});
