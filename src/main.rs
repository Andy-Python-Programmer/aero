//! # Aero
//! Aero is a new modern, unix based operating system. It is being developed for educational purposes.
//!
//! ## Code organization and architecture
//! The code is divided into different *modules*, each representing a *subsystem* of the kernel.
//!
//! **Notes**: \
//! - Unix: <https://en.wikipedia.org/wiki/Unix>

#![feature(custom_test_frameworks, core_intrinsics, asm, global_asm, llvm_asm)]
#![test_runner(crate::tests::test_runner)] // Attach our custom tests runner.
#![no_std] // Don't link the Rust standard library.
#![no_main] // Disable the rust entry point.

use bootloader::{entry_point, BootInfo};

mod gdt;
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

entry_point!(kernel_main);

fn kernel_main(_: &'static BootInfo) -> ! {
    gdt::init();
    log::info("Loaded GDT");

    log::info("Loaded IDT");
    log::info("Loaded PIT");
    log::info("Loaded PS/2 driver");
    log::info("Loaded paging");

    log::info("Initialized kernel");

    println!("\nHello World!");

    loop {}
}
