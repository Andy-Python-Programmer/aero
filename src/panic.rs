use crate::println;
use crate::vga::{
    color::{Color, ColorCode},
    rendy::RENDERER,
};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    RENDERER.lock().color_code = ColorCode::new(Color::White, Color::Blue);
    RENDERER.lock().clear_screen();

    println!(":(\n\n\n\n{}", info);

    loop {}
}
