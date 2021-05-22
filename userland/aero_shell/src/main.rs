#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

static TEST_BSS_NON_ZERO: usize = usize::MAX;
static TEST_BSS_ZEROED: usize = 0x00;

#[export_name = "_start"]
extern "C" fn main() {
    {
        assert_eq!(TEST_BSS_ZEROED, 0x00);
        assert_eq!(TEST_BSS_NON_ZERO, usize::MAX);
    }

    aero_syscall::sys_exit(0x00);

    loop {}
}

#[panic_handler]
extern "C" fn rust_begin_unwind(_: &PanicInfo) -> ! {
    loop {}
}
