use crate::prelude::*;
use crate::syscall;

pub mod process;
pub mod scheduler;

intel_fn!(
    #![cfg(target_pointer_width = "64")]

    pub extern "asm" fn jump_userland(address: u64) {
        "",
    }
);

/// Initialize userland.
pub fn init() {
    scheduler::init();
    syscall::init();
}
