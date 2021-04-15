use core::fmt::{self, Write};

use super::color::ColorCode;

use aero_boot::{FrameBuffer, FrameBufferInfo, PixelFormat};

use font8x8::UnicodeFonts;

use spin::{mutex::Mutex, Once};

static RENDY: Once<Mutex<Rendy>> = Once::new();

pub struct Rendy<'buffer> {
    buffer: &'buffer mut FrameBuffer,
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

impl<'buffer> Rendy<'buffer> {
    #[inline]
    fn new(buffer: &'buffer mut FrameBuffer) -> Self {
        let info = buffer.info();

        Self {
            buffer,
            info,
            x_pos: 0,
            y_pos: 0,
        }
    }

    #[inline]
    fn write_string(&mut self, string: &str) {
        for char in string.chars() {
            self.write_character(char)
        }
    }

    #[inline]
    fn width(&self) -> usize {
        self.info.horizontal_resolution
    }

    #[inline]
    fn height(&self) -> usize {
        self.info.vertical_resolution
    }

    fn write_character(&mut self, char: char) {
        match char {
            '\n' => self.new_line(),
            '\r' => self.carriage_return(),
            _ => {
                let char = font8x8::BASIC_FONTS.get(char).unwrap();

                if self.x_pos >= self.width() {
                    self.new_line();
                }

                if self.y_pos >= (self.height() - 8) {
                    self.clear_screen()
                }

                self.put_bytes(&char);
            }
        }
    }

    fn new_line(&mut self) {
        self.y_pos += 8;

        self.carriage_return()
    }

    #[inline]
    fn carriage_return(&mut self) {
        self.x_pos = 0;
    }

    fn put_bytes(&mut self, char: &[u8]) {
        for (y, byte) in char.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                let alpha = if *byte & (1 << bit) == 0 { 0 } else { 255 };

                self.put_pixel(self.x_pos + x, self.y_pos + y, alpha);
            }
        }

        self.x_pos += 8;
    }

    fn put_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.info.stride + x;

        let color = match self.info.pixel_format {
            PixelFormat::RGB => [intensity, intensity, intensity, 0],
            PixelFormat::BGR => [intensity / 2, intensity, intensity, 0],
            _ => unimplemented!(),
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        self.buffer.buffer_mut()[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
    }

    #[inline]
    fn clear_screen(&mut self) {
        self.buffer.buffer_mut().fill(0);
    }
}

impl<'buffer> fmt::Write for Rendy<'buffer> {
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

pub fn set_color_code(color_code: ColorCode) {}
pub fn clear_screen() {}

pub fn init(framebuffer: &'static mut FrameBuffer) {
    let mut rendy = Rendy::new(framebuffer);

    rendy.clear_screen();

    RENDY.call_once(|| Mutex::new(rendy));
}
