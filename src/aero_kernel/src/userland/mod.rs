use crate::syscall;

pub mod elf;
pub mod process;
pub mod scheduler;

extern "C" {
    pub fn mission_hello_world();
    // pub fn jump_userland() -> !;
}

/// Initialize userland.
pub fn init() {
    scheduler::init();
    syscall::init();
}
