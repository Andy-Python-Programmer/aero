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
    naked_functions,
    abi_x86_interrupt,
    alloc_error_handler,
    const_mut_refs,
    lang_items,
    panic_info_message,
    thread_local,
    const_fn_fn_ptr_basics
)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]

extern crate alloc;

use arch::interrupts::{PIC1_DATA, PIC2_DATA};
use arch::memory;

use userland::{process::Process, scheduler};
use utils::io;

mod acpi;
mod apic;
mod arch;
mod drivers;
mod logger;
mod rendy;
mod syscall;
mod tests;
mod time;
mod unwind;
mod userland;
mod utils;

use aero_boot::BootInfo;
use linked_list_allocator::LockedHeap;
use x86_64::{PhysAddr, VirtAddr};

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: LockedHeap = LockedHeap::empty();

const ASCII_INTRO: &str = r"
_______ _______ ______ _______    _______ ______ 
(_______|_______|_____ (_______)  (_______) _____)
 _______ _____   _____) )     _    _     ( (____  
|  ___  |  ___) |  __  / |   | |  | |   | \____ \ 
| |   | | |_____| |  \ \ |___| |  | |___| |____) )
|_|   |_|_______)_|   |_\_____/    \_____(______/ 
";

pub static mut LOL: VirtAddr = VirtAddr::zero();

#[export_name = "_start"]
extern "C" fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Initialize the COM ports before doing anything else.
    //
    // This will help printing panics before or when the debug renderer is initialized
    // if serial output is avaliable.
    drivers::uart_16550::init();

    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    unsafe {
        LOL = physical_memory_offset;
    }
    let rsdp_address = PhysAddr::new(boot_info.rsdp_address);

    let memory_regions = &boot_info.memory_regions;
    let framebuffer = &mut boot_info.framebuffer;

    rendy::init(framebuffer);
    logger::init();

    unsafe {
        arch::interrupts::disable_interrupts();

        arch::gdt::init();
        log::info!("Loaded GDT");

        arch::interrupts::init();
        log::info!("Loaded IDT");

        time::init();
        log::info!("Loaded PIT");

        drivers::mouse::init();
        log::info!("Loaded PS/2 driver");

        io::outb(PIC1_DATA, 0b11111000);
        io::outb(PIC2_DATA, 0b11101111);

        arch::interrupts::enable_interrupts();

        let (mut offset_table, mut frame_allocator) =
            memory::paging::init(physical_memory_offset, memory_regions);
        log::info!("Loaded paging");

        arch::memory::alloc::init_heap(&mut offset_table, &mut frame_allocator)
            .expect("Failed to initialize the heap.");
        log::info!("Loaded heap");

        apic::init(physical_memory_offset);
        log::info!("Loaded local apic");

        acpi::init(
            &mut offset_table,
            &mut frame_allocator,
            rsdp_address,
            physical_memory_offset,
        );
        log::info!("Loaded ACPI");

        drivers::pci::init(&mut offset_table, &mut frame_allocator);
        log::info!("Loaded PCI driver");

        userland::init();
        log::info!("Loaded userland");

        log::info!("Initialized kernel");

        println!("{}", ASCII_INTRO);

        print!("$ ");

        scheduler::get_scheduler().push(Process::from_function(userland::mission_hello_world));

        loop {
            arch::interrupts::halt();
        }
    }
}
