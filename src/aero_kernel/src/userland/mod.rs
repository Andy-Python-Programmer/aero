use crate::syscall;

pub mod process;
pub mod scheduler;

extern "C" {
    pub fn jump_userland(address: u64) -> !;
}

/// Initialize userland.
pub fn init() {
    scheduler::init();
    syscall::init();
}
