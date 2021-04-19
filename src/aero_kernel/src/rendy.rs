use core::fmt::{self, Write};

use aero_gfx::FrameBuffer;

use aero_gfx::debug::color::ColorCode;
use aero_gfx::debug::rendy::DebugRendy;

use spin::{mutex::Mutex, Once};

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

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    DEBUG_RENDY.get().unwrap().lock().write_fmt(args).unwrap();
}

pub fn set_color_code(color_code: ColorCode) {
    DEBUG_RENDY.get().unwrap().lock().set_color_code(color_code);
}

#[inline(always)]
pub fn is_initialized() -> bool {
    DEBUG_RENDY.get().is_some()
}

pub fn init(framebuffer: &'static mut FrameBuffer) {
    let info = framebuffer.info();
    let buffer = framebuffer.buffer_mut();

    let mut rendy = DebugRendy::new(buffer, info);

    rendy.clear_screen();

    DEBUG_RENDY.call_once(|| Mutex::new(rendy));
}
