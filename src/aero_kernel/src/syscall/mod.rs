//! System Calls are used to call a kernel service from user land.
//!
//! | %rax   | Name                    |
//! |--------|-------------------------|
//! | 0      | read                    |
//! | 1      | write                   |
//! | 2      | open                    |
//! | 3      | close                   |
//! | 60     | exit                    |
//!
//! **Notes**: <https://wiki.osdev.org/System_Calls>

pub mod fs;
pub mod process;
pub mod time;

use crate::arch::cpu::CPUState;
use crate::arch::interrupts::InterruptStackFrame;
use crate::{interrupt, println};

interrupt!(syscall, unsafe {
    let cpu_state = CPUState::new();

    match cpu_state.ax {
        0 => unimplemented!(),
        1 => unimplemented!(),
        2 => unimplemented!(),
        3 => unimplemented!(),
        60 => process::exit(),
        _ => println!("Invalid syscall"),
    }
});
