#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[export_name = "_start"]
extern "C" fn main() {
    loop {}
}

#[panic_handler]
extern "C" fn rust_begin_unwind(_: &PanicInfo) -> ! {
    loop {}
}
