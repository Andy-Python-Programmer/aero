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
    alloc_error_handler,
    const_mut_refs,
    lang_items,
    panic_info_message,
    thread_local,
    const_fn_fn_ptr_basics,
    decl_macro,
    global_asm
)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]

extern crate alloc;

use aero_boot::BootInfo;

use linked_list_allocator::LockedHeap;
use x86_64::VirtAddr;

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
mod prelude {
    pub use crate::rendy::{print, println};
}

use arch::interrupts;

use arch::interrupts::{PIC1_DATA, PIC2_DATA};
use arch::memory;

use userland::{process::Process, scheduler};
use utils::io;

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: LockedHeap = LockedHeap::empty();

static mut PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::zero();

const ASCII_INTRO: &str = r"
_______ _______ ______ _______    _______ ______ 
(_______|_______|_____ (_______)  (_______) _____)
 _______ _____   _____) )     _    _     ( (____  
|  ___  |  ___) |  __  / |   | |  | |   | \____ \ 
| |   | | |_____| |  \ \ |___| |  | |___| |____) )
|_|   |_|_______)_|   |_\_____/    \_____(______/ 
";

#[naked]
unsafe extern "C" fn mission_hello_world() {
    asm!("mov rax, 60; mov rdi, 0; syscall", options(noreturn));
}

#[export_name = "_start"]
extern "C" fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Initialize the COM ports before doing anything else.
    //
    // This will help printing panics before or when the debug renderer is initialized
    // if serial output is avaliable.
    drivers::uart_16550::init();

    unsafe {
        interrupts::disable_interrupts();
    }

    let memory_regions = &boot_info.memory_regions;
    let framebuffer = &mut boot_info.framebuffer;

    unsafe {
        PHYSICAL_MEMORY_OFFSET = boot_info.physical_memory_offset;
    }

    rendy::init(framebuffer);
    logger::init();

    arch::gdt::init();
    log::info!("Loaded GDT");

    interrupts::init();
    log::info!("Loaded IDT");

    time::init();
    log::info!("Loaded PIT");

    drivers::mouse::init();
    log::info!("Loaded PS/2 driver");

    unsafe {
        io::outb(PIC1_DATA, 0b11111000);
        io::outb(PIC2_DATA, 0b11101111);

        interrupts::enable_interrupts();
    }

    let apic_type = apic::init();
    log::info!(
        "Loaded local apic (x2apic={})",
        apic_type.supports_x2_apic()
    );

    let (mut offset_table, mut frame_allocator) =
        memory::paging::init(boot_info.physical_memory_offset, memory_regions);
    log::info!("Loaded paging");

    arch::memory::alloc::init_heap(&mut offset_table, &mut frame_allocator)
        .expect("Failed to initialize the heap.");
    log::info!("Loaded heap");

    acpi::init(
        &mut offset_table,
        &mut frame_allocator,
        boot_info.rsdp_address,
        boot_info.physical_memory_offset,
    );
    log::info!("Loaded ACPI");

    drivers::pci::init(&mut offset_table, &mut frame_allocator);
    log::info!("Loaded PCI driver");

    userland::init();
    log::info!("Loaded userland");

    apic::mark_bsp_ready(true);

    log::info!("Initialized kernel");

    prelude::println!("{}", ASCII_INTRO);
    prelude::print!("$ ");

    let hello_process = Process::from_function(mission_hello_world);

    scheduler::get_scheduler().push(hello_process);

    unsafe {
        // let ret: u64;

        // asm!("syscall", lateout("rax") ret, in("rax") 60, in("rdi") 1, lateout("rcx") _, lateout("r11") _);

        // println!("ret: {}", ret);

        loop {
            interrupts::halt();
        }
    }
}

#[no_mangle]
extern "C" fn kernel_ap_startup(cpu_id: u64) -> ! {
    log::info!("Starting CPU with id: {}", cpu_id);

    arch::gdt::init();
    log::info!("(cpu={}) Loaded GDT", cpu_id);

    interrupts::init();
    log::info!("(cpu={}) Loaded IDT", cpu_id);

    apic::mark_ap_ready(true);

    while !apic::is_bsp_ready() {
        interrupts::pause();
    }

    unsafe {
        loop {
            interrupts::disable_interrupts();

            if scheduler::reschedule() {
                interrupts::enable_interrupts();
            } else {
                interrupts::enable_interrupts_and_halt();
            }
        }
    }
}
