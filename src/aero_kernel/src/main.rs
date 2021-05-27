/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

//! # Aero
//! Aero is a new modern, unix based operating system. It is being developed for educational purposes.
//!
//! ## Code organization and architecture
//! The code is divided into different *modules*, each representing a *subsystem* of the kernel.
//!
//! ## Notes:
//! * <https://en.wikipedia.org/wiki/Unix>

#![feature(
    custom_test_frameworks,
    core_intrinsics,
    asm,
    alloc_error_handler,
    lang_items,
    panic_info_message,
    thread_local,
    decl_macro,
    global_asm,
    extern_types,
    new_uninit,
    box_syntax,
    const_btree_new // TODO: Do not abuse nightly rust :D
)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]

extern crate alloc;

use aero_boot::{BootInfo, UnwindInfo};

use linked_list_allocator::LockedHeap;
use spin::Once;
use x86_64::{registers, VirtAddr};

mod acpi;
mod apic;
mod arch;
mod drivers;
mod fs;
mod logger;
mod mem;
mod rendy;
mod syscall;
mod tests;
mod time;
mod tls;
mod unwind;
mod userland;
mod utils;
mod prelude {
    pub use crate::drivers::uart_16550::{serial_print, serial_println};
    pub use crate::mem::{memcmp, memcpy, memmove, memset};
    pub use crate::rendy::{print, println};
    pub use crate::utils::{
        const_unsafe, downcast, intel_asm, intel_fn, pop_fs, pop_preserved, pop_scratch, push_fs,
        push_preserved, push_scratch,
    };
}

use arch::interrupts;
use arch::interrupts::{PIC1_DATA, PIC2_DATA};

use utils::io;

use userland::scheduler;

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

#[no_mangle]
extern "C" fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    /*
     * NOTE: In this function we only want to initialize essential serivces, including
     * the task scheduler. Rest of the initializing (including kernel modules) should go
     * into the kernel main thread function instead.
     *
     * First of all make sure interrupts are disabled.
     */
    unsafe {
        interrupts::disable_interrupts();
    }

    /*
     * Initialize the COM ports before doing anything else.
     *
     * This will help printing panics and logs before or when the debug renderer
     * is initialized if serial output is avaliable.
     */
    drivers::uart_16550::init();
    logger::init();

    /*
     * Now that we have initialized basic logging we have to make sure that the
     * kernel is loaded in the higher half offset that we set in the linker script.
     */
    {
        let rip = registers::read_rip().as_u64();

        assert_eq!(rip & 0xffffffffffff0000, 0xffff800080000000);
    }

    unsafe {
        PHYSICAL_MEMORY_OFFSET = boot_info.physical_memory_offset;
    }

    UNWIND_INFO.call_once(|| boot_info.unwind_info);

    rendy::init(&mut boot_info.framebuffer);

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

    let mut offset_table = mem::paging::init(&boot_info.memory_regions).unwrap();
    log::info!("Loaded paging");

    mem::alloc::init_heap(&mut offset_table).expect("Failed to initialize the heap.");
    log::info!("Loaded heap");

    tls::init();
    log::info!("Loaded TLS");

    arch::gdt::init(boot_info.unwind_info.stack_top);
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

    acpi::init(
        &mut offset_table,
        boot_info.rsdp_address,
        boot_info.physical_memory_offset,
    );
    log::info!("Loaded ACPI");

    drivers::pci::init(&mut offset_table);
    log::info!("Loaded PCI driver");

    fs::init();
    log::info!("Loaded filesystem");

    userland::init();
    log::info!("Loaded userland");

    apic::mark_bsp_ready(true);

    log::info!("Initialized kernel");

    prelude::println!("{}", ASCII_INTRO);
    prelude::print!("$ ");

    /*
     * Now that all of the essential initialization is done we are going to schedule
     * the kernel main thread.
     *
     * TODO(Andy-Python-Programmer): Add support in the scheduler to run kernel processes
     * with ring 0 permissions:
     *
     * let init = Process::from_function(kernel_main_thread);
     * scheduler::get_scheduler().push(init);
     */

    userland::run(&mut offset_table).expect("Failed to run userspace shell");

    unsafe {
        loop {
            interrupts::disable_interrupts();

            if scheduler::get_scheduler().reschedule() {
                interrupts::enable_interrupts();
            } else {
                interrupts::enable_interrupts_and_halt();
            }
        }
    }
}

#[no_mangle]
extern "C" fn kernel_main_thread() {}

#[no_mangle]
extern "C" fn kernel_ap_startup(cpu_id: u64) -> ! {
    log::info!("Starting CPU with id: {}", cpu_id);

    while !apic::is_bsp_ready() {
        interrupts::pause();
    }

    unsafe {
        loop {
            interrupts::disable_interrupts();

            if scheduler::get_scheduler().reschedule() {
                interrupts::enable_interrupts();
            } else {
                interrupts::enable_interrupts_and_halt();
            }
        }
    }
}
