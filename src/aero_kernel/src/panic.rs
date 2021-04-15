use core::panic::PanicInfo;

use crate::println;
use crate::vga::rendy;
use crate::{
    arch::interrupts,
    vga::color::{Color, ColorCode},
};

#[panic_handler]
pub extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    rendy::set_color_code(ColorCode::new(Color::White, Color::Blue));
    rendy::clear_screen();

    let deafult_panic = &format_args!("");
    let panic_message = info.message().unwrap_or(deafult_panic);

    println!(
        "Kernel Panicked -> {}\n\n{}",
        info.location().unwrap(),
        panic_message,
    );

    loop {}
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
