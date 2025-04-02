// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

//! # Aero
//! Aero is a new modern, unix based operating system. It is being developed for educational
//! purposes.
//!
//! ## Code organization and architecture
//! The code is divided into different *modules*, each representing a *subsystem* of the kernel.
//!
//! ## Notes:
//! * <https://en.wikipedia.org/wiki/Unix>

#![feature(
    custom_test_frameworks, // https://github.com/rust-lang/rust/issues/50297
    alloc_error_handler, // https://github.com/rust-lang/rust/issues/51540
    lang_items, // No tracking issue
    decl_macro, // https://github.com/rust-lang/rust/issues/39412
    ptr_internals, // No tracking issue
    linked_list_cursors, // https://github.com/rust-lang/rust/issues/58533
    extern_types, // https://github.com/rust-lang/rust/issues/43467
    step_trait, // https://github.com/rust-lang/rust/issues/42168
    prelude_import, // No tracking issue
    allocator_api, // https://github.com/rust-lang/rust/issues/32838
    maybe_uninit_write_slice, // https://github.com/rust-lang/rust/issues/79995
    slice_ptr_get, // https://github.com/rust-lang/rust/issues/74265
    maybe_uninit_as_bytes, // https://github.com/rust-lang/rust/issues/93092
    const_trait_impl, // https://github.com/rust-lang/rust/issues/67792
    int_roundings, // https://github.com/rust-lang/rust/issues/88581
    naked_functions, // https://github.com/rust-lang/rust/issues/32408
    cfg_match, // https://github.com/rust-lang/rust/issues/115585
    associated_type_defaults,
    new_zeroed_alloc, // https://github.com/rust-lang/rust/issues/129396
    sync_unsafe_cell
)]
// TODO(andypython): can we remove the dependency of "prelude_import" and "lang_items"?
//     `lang_items`     => is currently used for the personality function (`rust_eh_personality`).
//     `prelude_import` => is currently just used to re-export alloc prelude. This just makes the
//                         files overall more readable.
#![allow(internal_features)]
#![deny(trivial_numeric_casts, unused_allocation)]
#![test_runner(crate::tests::test_runner)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![warn(clippy::needless_pass_by_value)]
#![deny(clippy::ptr_as_ptr)]

#[macro_use]
extern crate aero_proc;

mod prelude {
    #[allow(unused)]
    pub mod rust_2021 {
        // Since asm is used almost all over the kernel, its a better idea
        // to add it to the prelude.
        pub use core::arch::asm;
        pub use core::prelude::rust_2021::*;

        pub use alloc::string::String;

        pub use crate::rendy::dbg;
        pub use static_assertions::*;
    }
}

#[allow(unused)]
#[prelude_import]
pub use prelude::rust_2021::*;

extern crate alloc;

mod acpi;
mod arch;
mod cmdline;
mod drivers;
#[cfg(feature = "ci")]
mod emu;
mod fs;
mod logger;
mod mem;
mod modules;
mod net;
mod rendy;
mod socket;
mod syscall;
#[cfg(test)]
mod tests;
mod unwind;
mod userland;
mod utils;

use self::mem::alloc::LockedHeap;
use self::mem::paging::VirtAddr;

use self::arch::interrupts;
use self::userland::scheduler;

use self::userland::task::Task;

#[global_allocator]
static AERO_SYSTEM_ALLOCATOR: LockedHeap = LockedHeap::new_uninit();

static mut PHYSICAL_MEMORY_OFFSET: VirtAddr = VirtAddr::zero();

const IO_VIRTUAL_BASE: VirtAddr = VirtAddr::new(0xffffff0000000000);

const STT_GNU_IFUNC: u32 = 37;

pub fn relocate_self() {
    use xmas_elf::sections::SectionData;

    let unwind_info = unwind::UNWIND_INFO.get().unwrap();
    let kernel_elf = &unwind_info.kernel_elf;

    for section in kernel_elf.section_iter() {
        if let Ok(SectionData::Rela64(rela)) = section.get_data(kernel_elf) {
            for item in rela {
                if item.get_type() != STT_GNU_IFUNC {
                    continue;
                }

                let offset = unsafe { &mut *(item.get_offset() as *mut usize) };

                let resolver_ptr = item.get_addend() as *const u8;
                let resolver: fn() -> usize = unsafe { core::mem::transmute(resolver_ptr) };

                *offset = resolver();
            }
        }
    }
}

fn aero_main() -> ! {
    // NOTE: In this function we only want to initialize essential services, including
    // the task scheduler. Rest of the initializing (including kernel modules) should go
    // into the kernel main thread function instead.
    fs::init().unwrap();
    log::info!("loaded filesystem");

    crate::arch::time::init();
    log::info!("loaded timer");

    userland::scheduler::init();
    log::info!("loaded scheduler");

    #[cfg(target_arch = "x86_64")]
    crate::arch::apic::mark_bsp_ready(true);

    log::info!("initialized kernel");

    // Now that all of the essential initialization is done we are going to schedule
    // the kernel main thread.
    let init = Task::new_kernel(kernel_main_thread, true);
    let kdbg = Task::new_kernel(kernel_dbg_thread, true);
    scheduler::get_scheduler().register_task(init);
    scheduler::get_scheduler().register_task(kdbg);

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
    modules::init();
    log::info!("loaded kernel modules");

    net::init();
    log::info!("initialized networking stack");

    #[cfg(target_arch = "x86_64")]
    arch::enable_acpi();

    #[cfg(test)]
    test_main();

    if logger::enabled_rendy_debug() {
        #[cfg(not(test))]
        rendy::clear_screen(true);
        logger::set_rendy_debug(false);
    }

    #[cfg(test)]
    userland::run_tests().unwrap();

    #[cfg(not(test))]
    userland::run().unwrap();

    unreachable!()
}

fn kernel_dbg_thread() {
    use core::fmt::Write;

    use crate::drivers::uart::{self, LineStatus, COM_1};
    use crate::userland::task::TaskId;
    use crate::utils::sync::WaitQueue;

    uart::setup_interrupts();

    let input_wq = WaitQueue::new();
    let this_task = scheduler::current_thread();
    uart::register_listener(this_task.clone());

    let com_1 = COM_1.get().unwrap();

    loop {
        let mut input = String::new();

        loop {
            let mut com_1 = input_wq
                .block_on(com_1, |com_1| {
                    com_1.line_status().contains(LineStatus::INPUT_FULL)
                })
                .unwrap();

            let c = com_1.read_byte() as char;

            if c == '\r' {
                writeln!(com_1).unwrap();
                break;
            }

            input.push(c);
            write!(com_1, "{c}").unwrap();
        }

        let mut commands = input.split_whitespace();

        if let Some(name) = commands.next() {
            match name {
                "ps" => scheduler::get_scheduler().log_ptable(),
                "wake" => {
                    log::warn!("kdbg: forcefully waking up task");
                    let id = commands.next().unwrap().parse::<usize>().unwrap();
                    scheduler::get_scheduler()
                        .find_task(TaskId::new(id))
                        .unwrap()
                        .wake_up();
                }

                _ => log::warn!("kdbg: unknown command {name:?}"),
            }
        }
    }
}

extern "C" fn aero_ap_main(ap_id: usize) -> ! {
    log::info!("AP{}: Loaded userland", ap_id);

    loop {
        unsafe { interrupts::halt() }
    }
}
