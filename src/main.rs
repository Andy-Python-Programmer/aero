//! # Aero
//! Aero is a new modern, unix based operating system. It is being developed for educational purposes.
//!
//! ## Code organization and architecture
//! The code is divided into different *modules*, each representing a *subsystem* of the kernel.
//!
//! **Notes**: <https://en.wikipedia.org/wiki/Unix>

#![feature(
    custom_test_frameworks,
    core_intrinsics,
    asm,
    global_asm,
    llvm_asm,
    abi_x86_interrupt,
    alloc_error_handler
)]
#![test_runner(crate::tests::test_runner)] // Attach our custom tests runner.
#![no_std] // Don't link the Rust standard library.
#![no_main] // Disable the rust entry point.

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use drivers::mouse;
use interrupts::{enable_interrupts, PIC1_DATA, PIC2_DATA};
use memory::alloc::AeroSystemAllocator;
use utils::io;

mod drivers;
mod gdt;
mod interrupts;
mod memory;
mod panic;
mod pit;
mod tests;
mod utils;
mod vga;

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: AeroSystemAllocator = AeroSystemAllocator;

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
    unsafe {
        gdt::init();
        log::info("Loaded GDT");

        interrupts::init();
        log::info("Loaded IDT");

        pit::init();
        log::info("Loaded PIT");

        drivers::mouse::init();
        log::info("Loaded PS/2 driver");

        io::outb(PIC1_DATA, 0b11111000);
        io::outb(PIC2_DATA, 0b11101111);

        enable_interrupts();

        log::info("Loaded paging");

        memory::alloc::init_heap();
        log::info("Loaded Heap");

        log::info("Initialized kernel");

        println!("\nHello World!\n");
        print!("$ ");

        loop {
            mouse::process_mouse_packet();
        }
    }
}
