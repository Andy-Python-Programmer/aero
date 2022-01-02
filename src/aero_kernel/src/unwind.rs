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

use core::panic::PanicInfo;

use core::sync::atomic::{AtomicBool, Ordering};

use xmas_elf::sections::{SectionData, ShType};
use xmas_elf::symbol_table::Entry;
use xmas_elf::ElfFile;

use crate::mem::paging::{Translate, VirtAddr};
use crate::mem::AddressSpace;

use crate::logger;
use crate::rendy;

use crate::arch::interrupts;

static PANIC_HOOK_READY: AtomicBool = AtomicBool::new(false);

pub fn set_panic_hook_ready(yes: bool) {
    PANIC_HOOK_READY.store(yes, Ordering::SeqCst);
}

pub fn prepare_panic() {
    // Disable interrupts as we do not want to be interrupted while
    // we are unwinding the stack.
    unsafe {
        interrupts::disable_interrupts();
    }

    // Force unlock rendy and the logger ring buffer to prevent deadlock while
    // unwinding.
    unsafe {
        rendy::force_unlock();
        logger::force_unlock();
    }

    // Clear the screen if the rendy is initialized and enable
    // rendy debug in logger.
    if rendy::is_initialized() {
        rendy::clear_screen(true);
        logger::set_rendy_debug(true);
    }
}

pub fn unwind_stack_trace() {
    let mut address_space = AddressSpace::this();
    let offset_table = address_space.offset_page_table();

    let unwind_info = crate::UNWIND_INFO
        .get()
        .expect("unwind: failed to retrieve the unwind information");

    let kernel_slice: &[u8] = unsafe {
        core::slice::from_raw_parts(
            (crate::PHYSICAL_MEMORY_OFFSET + unwind_info.kernel_start).as_ptr(),
            unwind_info.kernel_size as usize,
        )
    };

    let kernel_elf = ElfFile::new(kernel_slice).expect("unwind: invalid kernel slice");
    let mut symbol_table = None;

    for section in kernel_elf.section_iter() {
        if section.get_type() == Ok(ShType::SymTab) {
            let section_data = section
                .get_data(&kernel_elf)
                .expect("Failed to get kernel section data information");

            if let SectionData::SymbolTable64(symtab) = section_data {
                symbol_table = Some(symtab);
            }
        }
    }

    let symbol_table = symbol_table.unwrap();
    let mut rbp: usize;

    unsafe {
        asm!("mov {}, rbp", out(reg) rbp);
    }

    // Make sure the RBP is not NULL. If it is then we cannot do the stack unwinding/tracing
    // as no frame pointers were emmited in this build. This should only occur if you
    // set the field `eliminate-frame-pointer` in the target file to true or manually resetting
    // the RBP to prevent backtrace to avoid address leaks (for example when jumping to userland).
    // If thats the case we return (resumes the parent function).
    if rbp == 0x00 {
        log::trace!("<empty backtrace>");
        return;
    }

    log::trace!("{:-^80}", " BACKTRACE ");

    for depth in 0..64 {
        if let Some(rip_rbp) = rbp.checked_add(core::mem::size_of::<usize>()) {
            if offset_table
                .translate_addr(VirtAddr::new(rip_rbp as u64))
                .is_none()
            {
                log::trace!("{:>2}: <guard page>", depth);
                break;
            }

            let rip = unsafe { *(rip_rbp as *const usize) };

            if rip == 0 {
                break;
            }

            unsafe {
                rbp = *(rbp as *const usize);
            }

            let mut name = None;

            for data in symbol_table {
                let st_value = data.value() as usize;
                let st_size = data.size() as usize;

                if rip >= st_value && rip < (st_value + st_size) {
                    let mangled_name = data.get_name(&kernel_elf).unwrap_or("<unknown>");
                    let demangled_name = rustc_demangle::demangle(mangled_name);

                    name = Some(demangled_name);
                }
            }

            if let Some(name) = name {
                log::trace!("{:>2}: 0x{:016x} - {}", depth, rip, name);
            } else {
                log::trace!("{:>2}: 0x{:016x} - <unknown>", depth, rip);
            }
        } else {
            // RBP has been overflowed...
            break;
        }
    }
}

#[cfg(feature = "ci")]
use crate::emu;

#[panic_handler]
extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    prepare_panic();

    let deafult_panic = &format_args!("");
    let panic_message = info.message().unwrap_or(deafult_panic);

    // Get the CPU ID where this panic happened and if PANIC_HOOK_READY is false
    // then we cannot get the CPU where this panic happened.
    let cpu_id = if PANIC_HOOK_READY.load(Ordering::SeqCst) {
        crate::tls::get_cpuid()
    } else {
        0x00
    };

    log::error!("cpu '{}' panicked at '{}'", cpu_id, panic_message);

    // Print the panic location if it is available.
    if let Some(panic_location) = info.location() {
        log::error!("{}", panic_location);
    }

    // Add a new line to make the stack trace more readable.
    log::error!("");

    unwind_stack_trace();

    #[cfg(feature = "ci")]
    emu::exit_qemu(emu::ExitStatus::Success);

    #[cfg(not(feature = "ci"))]
    unsafe {
        // Go into a halt loop to to save power.
        loop {
            interrupts::halt();
        }
    }
}

/// This function is automatically called after each unwinding cleanup routine finishes
/// executing. Our task here is to *resume* the unwinding procedure by figuring out where
/// we just came from and picking up where we left off.
#[allow(non_snake_case)]
#[no_mangle]
extern "C" fn _Unwind_Resume(unwind_context_ptr: usize) -> ! {
    log::debug!("{}", unwind_context_ptr);

    unsafe {
        interrupts::disable_interrupts();

        loop {
            interrupts::halt();
        }
    }
}

/// This function is the entry point for the unwinding process.
#[lang = "eh_personality"]
#[no_mangle]
extern "C" fn rust_eh_personality() -> ! {
    log::error!("Poisoned function `rust_eh_personaity` was invoked.");

    unsafe {
        interrupts::disable_interrupts();

        loop {
            interrupts::halt();
        }
    }
}
