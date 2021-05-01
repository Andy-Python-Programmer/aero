use core::panic::PanicInfo;

use crate::arch::interrupts;
use crate::{drivers::uart_16550::serial_println, rendy};

#[panic_handler]
pub extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    let deafult_panic = &format_args!("");
    let panic_message = info.message().unwrap_or(deafult_panic);

    if rendy::is_initialized() {
        rendy::clear_screen();

        log::error!("Kernel Panicked");
        log::error!("{}", info.location().unwrap());
        log::error!("{}", panic_message);
    } else {
        // Write the panic info to the COM 1 port if the debug renderer is not
        // yet initialized.

        serial_println!(
            "The kernel unexpectedly panicked before the debug renderer was initialized"
        );

        serial_println!(
            "{}",
            info.location().expect("Failed to get the panic location")
        );

        serial_println!("{}", panic_message);
    }

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
