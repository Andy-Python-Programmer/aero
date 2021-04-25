use core::fmt::{self, Write};

use aero_gfx::FrameBuffer;

use aero_gfx::debug::color::ColorCode;
use aero_gfx::debug::rendy::DebugRendy;

use spin::{mutex::Mutex, MutexGuard, Once};

static DEBUG_RENDY: Once<Mutex<DebugRendy>> = Once::new();

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::rendy::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! dbg {
    () => {
        $crate::println!("[{}:{}]", $crate::file!(), $crate::line!());
    };
    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::println!("[{}:{}] {} = {:#?}",
                    core::file!(), core::line!(), core::stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}

/// Get a mutable reference to the debug renderer.
fn get_debug_rendy() -> MutexGuard<'static, DebugRendy<'static>> {
    DEBUG_RENDY
        .get()
        .expect("Attempted to get the debug renderer before it was initialized")
        .lock()
}

/// Return [true] if the debug renderer is initialized.
#[inline]
pub fn is_initialized() -> bool {
    DEBUG_RENDY.get().is_some()
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    get_debug_rendy()
        .write_fmt(args)
        .expect("Failed to write to the framebuffer");
}

pub fn set_color_code(color_code: ColorCode) {
    get_debug_rendy().set_color_code(color_code);
}

pub fn clear_screen() {
    get_debug_rendy().clear_screen();
}

pub fn init(framebuffer: &'static mut FrameBuffer) {
    let info = framebuffer.info();
    let buffer = framebuffer.buffer_mut();

    let mut rendy = DebugRendy::new(buffer, info);

    rendy.clear_screen();

    DEBUG_RENDY.call_once(|| Mutex::new(rendy));
}
