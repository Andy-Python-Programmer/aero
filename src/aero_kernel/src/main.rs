/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
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
    alloc_error_handler,
    lang_items,
    panic_info_message,
    thread_local,
    decl_macro,
    ptr_internals,
    const_fn_trait_bound,
    linked_list_cursors,
    extern_types,
    new_uninit,
    box_syntax,
    const_fn_fn_ptr_basics,
    arc_new_cyclic,
    step_trait,
    const_btree_new,
    prelude_import
)]
#![deny(trivial_numeric_casts, unused_allocation)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]

mod prelude {
    pub mod rust_2021 {
        // Since asm is used almost all over the kernel, its a better idea
        // to add it to the prelude.
        pub use core::arch::asm;
        pub use core::prelude::rust_2021::*;
        pub use core::prelude::v1::*;
    }
}

#[prelude_import]
pub use prelude::rust_2021::*;

extern crate alloc;

mod acpi;
mod apic;
mod arch;
mod cmdline;
mod drivers;
#[cfg(feature = "ci")]
mod emu;
mod fs;
mod logger;
mod mem;
mod modules;
mod rendy;
mod socket;
mod syscall;
#[cfg(test)]
mod tests;
mod time;
mod tls;
mod unwind;
mod userland;
mod utils;

use stivale_boot::v2::*;

use self::mem::alloc::LockedHeap;
use self::mem::paging::VirtAddr;

use self::arch::interrupts;
use self::userland::scheduler;

use self::userland::task::Task;

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: LockedHeap = LockedHeap::new_uninit();

static mut PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::zero();
static UNWIND_INFO: spin::Once<&StivaleKernelFileV2Tag> = spin::Once::new();
static INITRD_MODULE: spin::Once<&StivaleModule> = spin::Once::new();

const IO_VIRTUAL_BASE: VirtAddr = VirtAddr::new(0xffffff0000000000);

fn aero_main() -> ! {
    // NOTE: In this function we only want to initialize essential serivces, including
    // the task scheduler. Rest of the initializing (including kernel modules) should go
    // into the kernel main thread function instead.
    drivers::mouse::init();
    log::info!("loaded PS/2 driver");

    time::init();
    log::info!("loaded PIT");

    fs::init().unwrap();
    log::info!("loaded filesystem");

    userland::init();
    log::info!("loaded userland");

    apic::mark_bsp_ready(true);

    log::info!("initialized kernel");

    // Now that all of the essential initialization is done we are going to schedule
    // the kernel main thread.
    let init = Task::new_kernel(kernel_main_thread, true);
    scheduler::get_scheduler().register_task(init);

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
    log::info!("loaded kernel modules");

    drivers::pci::init(&mut offset_table);
    log::info!("loaded PCI driver");

    fs::block::launch().unwrap();

    #[cfg(test)]
    test_main();

    #[cfg(not(test))]
    if logger::enabled_rendy_debug() {
        rendy::clear_screen(true);
        logger::set_rendy_debug(false);
    }

    #[cfg(not(test))]
    userland::run().unwrap();

    unreachable!()
}

#[no_mangle]
extern "C" fn kernel_ap_startup(ap_id: u64, stack_top_addr: VirtAddr) -> ! {
    log::debug!("booting CPU {}", ap_id);

    arch::gdt::init_boot();
    log::info!("AP{}: loaded boot GDT", ap_id);

    tls::init(ap_id as usize);
    log::info!("AP{}: loaded TLS", ap_id);

    arch::gdt::init(stack_top_addr);
    log::info!("AP{}: loaded GDT", ap_id);

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
