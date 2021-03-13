//! # Aero
//! Aero is a new modern, unix based operating system. It is being developed for educational purposes.
//!
//! ## Code organization and architecture
//! The code is divided into different *modules*, each representing a *subsystem* of the kernel.
//!
//! **Notes**: \
//! - Unix: <https://en.wikipedia.org/wiki/Unix>

#![feature(custom_test_frameworks)] // Enable custom test framework.
#![test_runner(crate::tests::test_runner)] // Attach our custom tests runner.
#![no_std] // Don't link the Rust standard library.
#![no_main] // Disable all Rust-level entry points.

mod panic;
mod tests;
mod vga;

mod log {
    use vga::color::*;

    use crate::vga::rendy::RENDERER;
    use crate::*;

    pub fn info(message: &str) {
        RENDERER.lock().color_code = ColorCode::new(Color::White, Color::Black);
        print!("[ ");
        RENDERER.lock().color_code = ColorCode::new(Color::LightGreen, Color::Black);
        print!("OK");
        RENDERER.lock().color_code = ColorCode::new(Color::White, Color::Black);
        println!(" ]        - {}", message);
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    log::info("Loaded GDT");
    log::info("Loaded IDT");
    log::info("Loaded PIT");
    log::info("Loaded PS/2 driver");
    log::info("Loaded paging");

    log::info("Initialized kernel");

    println!("\nHello World!");

    loop {}
}
