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
    abi_x86_interrupt,
    alloc_error_handler,
    const_mut_refs,
    lang_items
)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]

extern crate alloc;

use arch::interrupts::{PIC1_DATA, PIC2_DATA};
use arch::memory::{alloc::AeroSystemAllocator, paging};
use bootloader::{entry_point, BootInfo};
use utils::{io, memory::Locked};

mod acpi;
mod arch;
mod drivers;
mod elf;
mod panic;
mod pit;
mod syscall;
mod tests;
mod utils;
mod vga;

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: Locked<AeroSystemAllocator> = Locked::new(AeroSystemAllocator::new());

const ASCII_INTRO: &'static str = r"
_______ _______ ______ _______    _______ ______ 
(_______|_______|_____ (_______)  (_______) _____)
 _______ _____   _____) )     _    _     ( (____  
|  ___  |  ___) |  __  / |   | |  | |   | \____ \ 
| |   | | |_____| |  \ \ |___| |  | |___| |____) )
|_|   |_|_______)_|   |_\_____/    \_____(______/ 
";

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

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    unsafe {
        arch::interrupts::disable_interrupts();

        arch::gdt::init();
        log::info("Loaded GDT");

        arch::interrupts::init();
        log::info("Loaded IDT");

        pit::init();
        log::info("Loaded PIT");

        drivers::mouse::init();
        log::info("Loaded PS/2 driver");

        io::outb(PIC1_DATA, 0b11111000);
        io::outb(PIC2_DATA, 0b11101111);

        arch::interrupts::enable_interrupts();

        let (mut offset_table, mut frame_allocator) = paging::init(&boot_info);
        log::info("Loaded paging");

        acpi::init(&mut offset_table, &mut frame_allocator);
        log::info("Loaded ACPI");

        drivers::pci::init(&mut offset_table, &mut frame_allocator);
        log::info("Loaded PCI driver");

        arch::memory::alloc::init_heap(&mut offset_table, &mut frame_allocator)
            .expect("Failed to initialize the heap.");
        log::info("Loaded heap");

        log::info("Initialized kernel");

        println!("{}", ASCII_INTRO);

        print!("$ ");

        loop {
            arch::interrupts::halt();
        }
    }
}
