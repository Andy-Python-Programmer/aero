use core::fmt::{self, Write};

use super::{
    buffer::{Buffer, ScreenChar, BUFFER_HEIGHT, BUFFER_WIDTH},
    color::{Color, ColorCode},
};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref RENDERER: spin::Mutex<Rendy> = spin::Mutex::new(Rendy {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}
pub struct Rendy {
    column_position: usize,
    pub color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Rendy {
    pub fn string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.byte(byte),
                _ => self.byte(0xfe),
            }
        }
    }

    pub fn byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                let color_code = self.color_code;

                self.buffer.chars[row][col].write(ScreenChar {
                    character: byte,
                    color_code,
                });

                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }

        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            character: b' ',
            color_code: self.color_code,
        };

        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }

    pub fn clear_screen(&mut self) {
        let blank = ScreenChar {
            character: b' ',
            color_code: self.color_code,
        };

        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                self.buffer.chars[row][col].write(blank);
            }
        }

        self.column_position = 0;
    }

    pub fn clear_current(&mut self) {
        unimplemented!();
    }
}

impl fmt::Write for Rendy {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.string(s);
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

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    RENDERER.lock().write_fmt(args).unwrap();
}
