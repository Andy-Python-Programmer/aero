#![no_std] // Don't link the Rust standard library.
#![no_main] // Disable all Rust-level entry points.

mod vga;

use core::panic::PanicInfo;

use vga::{
    buffer::Buffer,
    color::{Color, ColorCode},
    rendy::Rendy,
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut rendy = Rendy::new(0, ColorCode::new(Color::Yellow, Color::Blue), unsafe {
        &mut *(0xb8000 as *mut Buffer)
    });

    rendy.string("Hello World!");

    loop {}
}
