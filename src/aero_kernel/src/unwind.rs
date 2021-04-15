use core::panic::PanicInfo;

use crate::arch::interrupts;

#[panic_handler]
pub extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    let deafult_panic = &format_args!("");
    let panic_message = info.message().unwrap_or(deafult_panic);

    log::error!("Kernel Panicked");
    log::error!("{}", info.location().unwrap());
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
