use core::fmt::{self, Write};

use aero_boot::{BootInfo, FrameBuffer, FrameBufferInfo, PixelFormat};
use spin::{Mutex, Once};

pub static RENDY: Once<Mutex<Rendy>> = Once::new();

pub struct Rendy {
    frame_buffer: &'static mut FrameBuffer,
    info: FrameBufferInfo,
}

impl Rendy {
    #[inline]
    fn new(frame_buffer: &'static mut FrameBuffer, info: FrameBufferInfo) -> Self {
        Self { frame_buffer, info }
    }

    pub fn write_string(&mut self, string: &str) {
        let y = 50;

        for x in 0..self.info.vertical_resolution / 2 * 4 {
            self.put_pixel(x, y, 255, 255, 255);
        }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        unsafe {
            self.frame_buffer
                .write_value(((y * self.info.stride) + x) * 4, i32::MAX);
        }
    }
}

impl fmt::Write for Rendy {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_string(string);

        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga::rendy::_print(format_args!($($arg)*)));
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
    RENDY.get().unwrap().lock().write_fmt(args).unwrap();
}

pub fn clear_screen() {}

pub fn init(boot_info: &'static mut BootInfo) {
    let rendy = Mutex::new(Rendy::new(
        &mut boot_info.frame_buffer,
        boot_info.frame_buffer_info,
    ));

    RENDY.call_once(|| rendy);
}
