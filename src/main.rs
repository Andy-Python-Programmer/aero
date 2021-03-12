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

use vga::{
    buffer::Buffer,
    color::{Color, ColorCode},
    rendy::Rendy,
};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut rendy = Rendy::new(0, ColorCode::new(Color::Yellow, Color::Blue), unsafe {
        &mut *(0xb8000 as *mut Buffer)
    });

    rendy.string("Hello World!");

    loop {}
}
