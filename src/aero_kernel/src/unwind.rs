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

use core::mem;
use core::panic::PanicInfo;

use xmas_elf::sections::{SectionData, ShType};
use xmas_elf::symbol_table::Entry;
use xmas_elf::ElfFile;

use crate::rendy;

use crate::arch::interrupts;
use crate::{PHYSICAL_MEMORY_OFFSET, UNWIND_INFO};

pub fn unwind_stack_trace() {
    let unwind_info = UNWIND_INFO
        .get()
        .expect("Failed to retrieve the unwind information");

    let kernel_slice: &[u8] = unsafe {
        core::slice::from_raw_parts(
            (unwind_info.kernel_base + PHYSICAL_MEMORY_OFFSET.as_u64()).as_ptr(),
            unwind_info.kernel_size,
        )
    };

    let kernel_elf = ElfFile::new(kernel_slice).expect("Invalid kernel slice");
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

    /*
     * Make sure rbp is not NULL. If it is then we cannot do the stack unwinding/tracing
     * as no frame pointers were emmited in this build. This should only occur if you
     * set the field `eliminate-frame-pointer` in the target file to true. If thats the case
     * we return (resumes the parent function).
     */
    if rbp == 0 {
        log::error!("Frame pointers were not emmited in this build");
        log::error!("Unable to unwind the stack");

        return;
    }

    log::error!("Stack backtrace:");

    let mut depth = 0;

    /*
     * The iteration goes up till the maximum of 64 frames.
     */
    for _ in 0..64 {
        if let Some(rip_rbp) = rbp.checked_add(mem::size_of::<usize>()) {
            let rip = unsafe { *(rip_rbp as *const usize) };

            /*
             * Check if the RIP register is 0 and that means that we are done with the
             * stack trace so break out of the loop.
             */
            if rip == 0 {
                break;
            }

            unsafe {
                rbp = *(rbp as *const usize);
            }

            for data in symbol_table {
                let st_value = data.value() as usize;
                let st_size = data.size() as usize;

                if rip >= st_value && rip < (st_value + st_size) {
                    let mangled_name = data.get_name(&kernel_elf).expect("Oh No!");
                    let demangled_name = rustc_demangle::demangle(mangled_name);

                    log::error!("\t{}:    {:#x} - {}", depth, rip, demangled_name);
                }
            }

            depth += 1;
        } else {
            /*
             * If the checked addition fails that means the RBP has overflowed. So just break
             * out.
             */
            log::error!("RBP overflowed => {:#x}", rbp);

            break;
        }
    }
}

#[panic_handler]
extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    let deafult_panic = &format_args!("");
    let panic_message = info.message().unwrap_or(deafult_panic);

    if rendy::is_initialized() {
        rendy::clear_screen();
    }

    log::error!("thread 'main' panicked at '{}'", panic_message);

    if let Some(panic_location) = info.location() {
        log::error!("{}", panic_location);
    }

    /*
     * Just to make the stack trace pretty. The programmer should be *very*
     * stressed at this point.
     */
    log::error!("");

    unwind_stack_trace();

    unsafe {
        interrupts::disable_interrupts();

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
