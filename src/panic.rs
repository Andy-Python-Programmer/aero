use core::panic::PanicInfo;

use crate::vga::{
    color::{Color, ColorCode},
    rendy::RENDERER,
};
use crate::{interrupts, println};

#[panic_handler]
pub extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    RENDERER.lock().color_code = ColorCode::new(Color::White, Color::Blue);
    RENDERER.lock().clear_screen();

    println!(":(\n\n\n\n{}", info);

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
