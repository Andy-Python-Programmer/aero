/*
 * Copyright (C) 2021 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
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
    ptr_internals,
    const_fn_trait_bound,
    linked_list_cursors,
    extern_types,
    new_uninit,
    box_syntax,
    const_fn_fn_ptr_basics,
    arc_new_cyclic,
    step_trait,
    const_btree_new
)]
#![deny(trivial_numeric_casts, unused_allocation)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]

extern crate alloc;

mod acpi;
mod apic;
mod arch;
mod drivers;
mod fs;
mod logger;
mod mem;
mod modules;
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
    pub use crate::rendy::{print, println};
    pub use crate::utils::{
        const_unsafe, intel_asm, intel_fn, pop_fs, pop_preserved, pop_scratch, push_fs,
        push_preserved, push_scratch,
    };
}

use stivale_boot::v2::*;

use self::mem::alloc::LockedHeap;
use self::mem::paging::{PhysAddr, VirtAddr};

use self::arch::interrupts;
use self::userland::scheduler;

use self::userland::task::Task;

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: LockedHeap = LockedHeap::new_uninit();

static mut PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::zero();

#[repr(C, align(4096))]
struct P2Align12<T>(T);

const STACK_SIZE: usize = 4096 * 16;

/// We need to tell the stivale bootloader where we want our stack to be.
/// We are going to allocate our stack as an uninitialised array in .bss.
static STACK: P2Align12<[u8; STACK_SIZE]> = P2Align12([0; STACK_SIZE]);

/// We are now going to define a framebuffer header tag. This tag tells the bootloader that
/// we want a graphical framebuffer instead of a CGA-compatible text mode. Omitting this tag will
/// make the bootloader default to text mode, if available.
static FRAMEBUFFER_TAG: StivaleFramebufferHeaderTag = StivaleFramebufferHeaderTag::new()
    .framebuffer_bpp(24)
    .next((&PAGING_TAG as *const Stivale5LevelPagingHeaderTag).cast());

/// We are now going to define a level 5 paging header tag. This tag tells the bootloader to
/// enable the LEVEL_5_PAGING bit in the Cr4 register. This is not possible to implement in the kernel
/// as we can only enable it in protected mode.
static PAGING_TAG: Stivale5LevelPagingHeaderTag = Stivale5LevelPagingHeaderTag::new();

/// The stivale2 specification says we need to define a "header structure".
/// This structure needs to reside in the .stivale2hdr ELF section in order
/// for the bootloader to find it. We use the #[linker_section] and #[used] macros to
/// tell the compiler to put the following structure in said section.
#[link_section = ".stivale2hdr"]
#[no_mangle]
#[used]
static STIVALE_HDR: StivaleHeader = StivaleHeader::new()
    .stack(&STACK.0[STACK_SIZE - 4096] as *const u8)
    .tags((&FRAMEBUFFER_TAG as *const StivaleFramebufferHeaderTag).cast());

#[thread_local]
static mut CPU_ID: u64 = 0x00;

#[no_mangle]
extern "C" fn kernel_main(boot_info: &'static StivaleStruct) -> ! {
    let mmap_tag = boot_info
        .memory_map()
        .expect("Aero requires the bootloader to provide a non-null memory map tag");

    let rsdp_tag = boot_info
        .rsdp()
        .expect("Aero requires the bootloader to provided a non-null rsdp tag");

    let framebuffer_tag = boot_info
        .framebuffer()
        .expect("Aero requires the bootloader to provide a non-null framebuffer tag");

    let rsdp_address = PhysAddr::new(rsdp_tag.rsdp);

    // Note: STACK_SIZE - 1 points to the last u8 in the array, i.e.
    // it's guaranteed to be at an address with its least significant bit
    // being a 1, i.e. it never has an alignment greater than 1. STACK_SIZE - 4096
    // points to the last u8 in STACK, that is aligned to 4096.
    let stack_top_addr = VirtAddr::new((&STACK.0[STACK_SIZE - 4096] as *const u8) as _);

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

    if mem::paging::level_5_paging_enabled() {
        unsafe { PHYSICAL_MEMORY_OFFSET = VirtAddr::new(0xff00000000000000) }
    } else {
        unsafe { PHYSICAL_MEMORY_OFFSET = VirtAddr::new(0xffff800000000000) }
    }

    arch::init_cpu();

    /*
     * Initialize the COM ports before doing anything else.
     *
     * This will help printing panics and logs before or when the debug renderer
     * is initialized if serial output is avaliable.
     */
    drivers::uart_16550::init();
    logger::init();

    rendy::init(framebuffer_tag);

    arch::gdt::init_boot();
    log::info!("Loaded bootstrap GDT");

    interrupts::init();
    log::info!("Loaded IDT");

    drivers::mouse::init();
    log::info!("Loaded PS/2 driver");

    let apic_type = apic::init();
    log::info!(
        "Loaded local apic (x2apic={})",
        apic_type.supports_x2_apic()
    );

    let mut offset_table = mem::paging::init(mmap_tag).unwrap();
    log::info!("Loaded paging");

    mem::alloc::init_heap(&mut offset_table).expect("Failed to initialize the heap.");
    log::info!("Loaded heap");

    tls::init();
    log::info!("Loaded TLS");

    arch::gdt::init(stack_top_addr);
    log::info!("Loaded GDT");

    acpi::init(rsdp_address, unsafe { PHYSICAL_MEMORY_OFFSET });
    log::info!("Loaded ACPI");

    time::init();
    log::info!("Loaded PIT");

    fs::init().unwrap();
    log::info!("Loaded filesystem");

    userland::init();
    log::info!("Loaded userland");

    apic::mark_bsp_ready(true);

    log::info!("Initialized kernel");

    /*
     * Now that all of the essential initialization is done we are going to schedule
     * the kernel main thread.
     */
    let init = Task::new_kernel(kernel_main_thread, true);
    scheduler::get_scheduler().register_task(init);

    /*
     * NOTE: We need to enable interrupts after we have initialized TLS and GDT
     * as the PTI context switch functions depend on thread local globals.
     */
    unsafe {
        interrupts::enable_interrupts();
    }

    // Pre-scheduler init done. Now we are waiting for the main kernel
    // thread to be scheduled.
    loop {
        unsafe { interrupts::halt() }
    }
}

fn kernel_main_thread() {
    let mut address_space = mem::AddressSpace::this();
    let mut offset_table = address_space.offset_page_table();

    modules::init();
    log::info!("Loaded kernel modules");

    drivers::pci::init(&mut offset_table);
    log::info!("Loaded PCI driver");

    userland::run();

    loop {}
}

#[no_mangle]
extern "C" fn kernel_ap_startup(ap_id: u64, stack_top_addr: VirtAddr) -> ! {
    log::debug!("Booting CPU {}", ap_id);

    tls::init();
    log::info!("AP{}: Loaded TLS", ap_id);

    arch::gdt::init(stack_top_addr);
    log::info!("AP{}: Loaded GDT", ap_id);

    unsafe {
        CPU_ID = ap_id; // Set the local cpu id global to the AP id provided in the AP bootinfo
    }

    apic::mark_ap_ready(true);

    while !apic::is_bsp_ready() {
        interrupts::pause();
    }

    userland::init_ap();
    log::info!("AP{}: Loaded userland", ap_id);

    loop {
        unsafe { interrupts::halt() }
    }
}
