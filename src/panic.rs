use core::panic::PanicInfo;

// use crate::arch::interrupts;
// use crate::println;
// use crate::vga::{
//     color::{Color, ColorCode},
//     rendy::RENDERER,
// };

#[panic_handler]
pub extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    // RENDERER.lock().color_code = ColorCode::new(Color::White, Color::Blue);
    // RENDERER.lock().clear_screen();

    // let deafult_panic = &format_args!("");
    // let panic_message = info.message().unwrap_or(deafult_panic);

    // println!(
    //     "Kernel Panicked -> {}\n\n{}",
    //     info.location().unwrap(),
    //     panic_message,
    // );

    loop {}
}

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() -> ! {
    loop {
        // unsafe {
        // interrupts::halt();
        // }
    }
}
