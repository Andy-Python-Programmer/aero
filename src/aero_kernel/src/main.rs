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
    lang_items,
    panic_info_message,
    thread_local,
    decl_macro,
    global_asm,
    extern_types,
    new_uninit
)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]

extern crate alloc;

use aero_boot::{BootInfo, UnwindInfo};

use linked_list_allocator::LockedHeap;
use spin::Once;
use x86_64::VirtAddr;

mod acpi;
mod apic;
mod arch;
mod boot;
mod drivers;
mod fs;
mod logger;
mod rendy;
mod syscall;
mod tests;
mod time;
mod tls;
mod unwind;
mod userland;
mod utils;
mod prelude {
    pub use crate::rendy::{print, println};
    pub use crate::utils::*;
}

use arch::interrupts;
use arch::memory;

use utils::io;

use arch::interrupts::{PIC1_DATA, PIC2_DATA};
use arch::memory::pti::PTI_CONTEXT_STACK_ADDRESS;

use userland::{process::Process, scheduler};

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: LockedHeap = LockedHeap::empty();

static mut PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::zero();
static UNWIND_INFO: Once<UnwindInfo> = Once::new();

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

#[no_mangle]
extern "C" fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    /*
     * First of all make sure interrupts are disabled.
     */
    unsafe {
        interrupts::disable_interrupts();
    }

    /*
     * Initialize the COM ports before doing anything else.
     *
     * This will help printing panics before or when the debug renderer is initialized
     * if serial output is avaliable.
     */
    drivers::uart_16550::init();

    unsafe {
        PHYSICAL_MEMORY_OFFSET = boot_info.physical_memory_offset;
    }

    UNWIND_INFO.call_once(|| boot_info.unwind_info);

    rendy::init(&mut boot_info.framebuffer);
    logger::init();

    arch::gdt::init_boot();
    log::info!("Loaded bootstrap GDT");

    interrupts::init();
    log::info!("Loaded IDT");

    time::init();
    log::info!("Loaded PIT");

    drivers::mouse::init();
    log::info!("Loaded PS/2 driver");

    let apic_type = apic::init();
    log::info!(
        "Loaded local apic (x2apic={})",
        apic_type.supports_x2_apic()
    );

    let (mut offset_table, mut frame_allocator) =
        memory::paging::init(boot_info.physical_memory_offset, &boot_info.memory_regions);
    log::info!("Loaded paging");

    arch::memory::alloc::init_heap(&mut offset_table, &mut frame_allocator)
        .expect("Failed to initialize the heap.");
    log::info!("Loaded heap");

    tls::init();
    log::info!("Loaded TLS");

    arch::gdt::init();
    log::info!("Loaded GDT");

    /*
     * NOTE: We need to enable interrupts after we have initialized TLS and GDT
     * as the PTI context switch functions depend on thread local globals.
     */
    unsafe {
        io::outb(PIC1_DATA, 0b11111000);
        io::outb(PIC2_DATA, 0b11101111);

        interrupts::enable_interrupts();
    }

    unsafe {
        *(0xdeadbeef as *mut u32) = 69;
    }

    acpi::init(
        &mut offset_table,
        &mut frame_allocator,
        boot_info.rsdp_address,
        boot_info.physical_memory_offset,
    );
    log::info!("Loaded ACPI");

    drivers::pci::init(&mut offset_table, &mut frame_allocator);
    log::info!("Loaded PCI driver");

    fs::init();
    log::info!("Loaded filesystem");

    userland::init();
    log::info!("Loaded userland");

    apic::mark_bsp_ready(true);

    log::info!("Initialized kernel");

    prelude::println!("{}", ASCII_INTRO);
    prelude::print!("$ ");

    let hello_process = Process::from_function(mission_hello_world);

    scheduler::get_scheduler().push(hello_process);

    unsafe {
        aero_syscall::sys_exit(1);

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
