use core::fmt::Write;
use core::panic::PanicInfo;

use crate::rendy;
use crate::{arch::interrupts, drivers::uart_16550};

#[panic_handler]
pub extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    let deafult_panic = &format_args!("");
    let panic_message = info.message().unwrap_or(deafult_panic);

    if rendy::is_initialized() {
        log::error!("Kernel Panicked");
        log::error!("{}", info.location().unwrap());
        log::error!("{}", panic_message);
    } else {
        // Write the panic info to the com 1 port if the debug renderer is not
        // yet initialized.

        let mut com_1 = uart_16550::get_com_1();

        writeln!(
            com_1,
            "The kernel unexpectedly panicked before the debug renderer was initialized."
        )
        .expect("Failed to write to the COM1 port");

        writeln!(
            com_1,
            "{}",
            info.location().expect("Failed to get panic the location")
        )
        .expect("Failed to write to the COM1 port");

        writeln!(com_1, "{}", panic_message).expect("Failed to write to the COM1 port");
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
