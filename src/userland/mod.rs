pub mod elf;
pub mod scheduler;
pub mod task;

/// Initialize userland.
pub fn init() {}

/// Jump to userland.
pub unsafe fn jump_to_userland(address: u64) {
    asm!(include_str!("userland.s"), in("rdi") address, in("ax") 0x1b, in("dx") 0x23, in("rsi") 0x00);
}
