use core::fmt::{self, Write};

use super::color::ColorCode;

use aero_boot::{FrameBuffer, FrameBufferInfo};

use font8x8::UnicodeFonts;

use spin::{mutex::Mutex, Once};

static RENDY: Once<Mutex<Rendy>> = Once::new();

pub struct Rendy<'buffer> {
    buffer: &'buffer mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    color: ColorCode,
}

impl<'buffer> Rendy<'buffer> {
    #[inline]
    fn new(buffer: &'buffer mut FrameBuffer) -> Self {
        let info = buffer.info();

        Self {
            buffer: buffer.buffer_mut(),
            info,
            x_pos: 0,
            y_pos: 0,
            color: ColorCode::new(0xFFFFFF, 0x00),
        }
    }

    fn write_string(&mut self, string: &str) {
        for char in string.chars() {
            self.write_character(char)
        }
    }

    #[inline(always)]
    fn width(&self) -> usize {
        self.info.horizontal_resolution
    }

    #[inline(always)]
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
        self.y_pos += 16;

        self.carriage_return()
    }

    #[inline(always)]
    fn carriage_return(&mut self) {
        self.x_pos = 0;
    }

    fn put_bytes(&mut self, char: &[u8]) {
        for (y, byte) in char.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                let background = if *byte & (1 << bit) == 0 { true } else { false };

                if background {
                    self.put_pixel(self.x_pos + x, self.y_pos + y, self.color.get_background());
                } else {
                    self.put_pixel(self.x_pos + x, self.y_pos + y, self.color.get_foreground());
                }
            }
        }

        self.x_pos += 8;
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        let pixel_offset = y * self.info.stride + x;

        let color = [
            (color & 255u32) as u8,
            ((color >> 8u32) & 255) as u8,
            ((color >> 16u32) & 255) as u8,
            0,
        ];

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        self.buffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
    }

    #[inline(always)]
    fn set_color_code(&mut self, color: ColorCode) {
        self.color = color;
    }

    fn clear_screen(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;

        self.buffer.fill(0);
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

pub fn set_color_code(color_code: ColorCode) {
    RENDY.get().unwrap().lock().set_color_code(color_code);
}

pub fn init(framebuffer: &'static mut FrameBuffer) {
    let mut rendy = Rendy::new(framebuffer);

    rendy.clear_screen();

    RENDY.call_once(|| Mutex::new(rendy));
}
