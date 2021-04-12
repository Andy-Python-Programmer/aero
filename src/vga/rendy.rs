use core::fmt::{self, Write};

use aero_boot::{BootInfo, FrameBufferInfo, PixelFormat};
use spin::{Mutex, Once};

static RENDY: Once<Mutex<Rendy>> = Once::new();

pub struct Rendy {
    frame_buffer: &'static mut [u8],
    info: FrameBufferInfo,
}

impl Rendy {
    pub fn write_string(&mut self, string: &str) {
        let y = 50;

        for x in 0..self.info.horizontal_resolution / 2 {
            self.put_pixel(x, y, 255);
        }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.info.stride + x;

        let color = match self.info.pixel_format {
            PixelFormat::RGB => [intensity, intensity, intensity / 2, 0],
            PixelFormat::BGR => [intensity / 2, intensity, intensity, 0],
            _ => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
        };

        let byte_offset = pixel_offset * 4;

        self.frame_buffer[byte_offset..(byte_offset + 4)].copy_from_slice(&color[..4]);
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

pub fn init(boot_info: &'static mut BootInfo) {
    let frame_buffer = &mut boot_info.frame_buffer;
    let info = boot_info.frame_buffer_info;

    let rendy = Mutex::new(Rendy { frame_buffer, info });

    RENDY.call_once(|| rendy);
}
