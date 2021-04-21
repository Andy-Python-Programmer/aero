use crate::syscall;

pub mod elf;
pub mod process;
pub mod scheduler;

/// Initialize userland.
pub fn init() {
    scheduler::init();
    syscall::init();
}
