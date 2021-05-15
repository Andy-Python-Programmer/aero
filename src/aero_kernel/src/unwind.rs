use core::mem;
use core::panic::PanicInfo;

use x86_64::VirtAddr;

use xmas_elf::ElfFile;

use crate::prelude::*;
use crate::rendy;

use crate::arch::interrupts;
use crate::{PHYSICAL_MEMORY_OFFSET, UNWIND_INFO};

#[no_mangle]
pub extern "C" fn exception_begin_unwind() {
    let mut rbp: usize;

    unsafe {
        asm!("mov {}, rbp", out(reg) rbp);
    }

    log::error!("RBP: {:#x}", rbp);

    let unwind_info = UNWIND_INFO.get().expect("o_O");
    let kernel_slice: &[u8] = unsafe {
        core::slice::from_raw_parts(
            (VirtAddr::new_unsafe(0x100000) + PHYSICAL_MEMORY_OFFSET.as_u64()).as_ptr(),
            unwind_info.kernel_size,
        )
    };

    let kernel_elf = ElfFile::new(kernel_slice).expect("o_O");
    let kernel_elf_p2 = kernel_elf.header.pt2;

    for _ in 0..kernel_elf_p2.sh_count() {
        if let Some(rip_rbp) = rbp.checked_add(mem::size_of::<usize>()) {
            let rip = unsafe { *(rip_rbp as *const usize) };

            /*
             * Check if the RIP register is 0 and that means it was an empty return. So just
             * break out.
             */
            if rip == 0 {
                log::error!("Empty return => (RIP == 0)");

                break;
            }

            /*
             * If we make through here, that means we can do the stack unwinding using the
             * unwind info struct that Aero's bootloader gave us.
             */
            log::error!("RIP: {:#x}", rip);
            log::error!("RBP: {:#x}", rbp);
        } else {
            /*
             * If the checked addition fails that means the RBP has overflowed. So just break
             * out.
             */
            log::error!("RBP overflowed => {:#x}", rbp);

            break;
        }
    }

    unsafe {
        interrupts::disable_interrupts();

        loop {
            interrupts::halt();
        }
    }
}

#[panic_handler]
pub extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    let deafult_panic = &format_args!("");
    let panic_message = info.message().unwrap_or(deafult_panic);

    if rendy::is_initialized() {
        rendy::clear_screen();
    }

    log::error!("Kernel Panicked");

    if let Some(panic_location) = info.location() {
        log::error!("{}", panic_location);
    }

    log::error!("{}", panic_message);

    unsafe {
        interrupts::disable_interrupts();

        loop {
            interrupts::halt();
        }
    }
}

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {
        unsafe {
            interrupts::halt();
        }
    }
}
