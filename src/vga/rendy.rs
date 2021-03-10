use super::{
    buffer::{Buffer, ScreenChar, BUFFER_HEIGHT, BUFFER_WIDTH},
    color::ColorCode,
};

pub struct Rendy {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}

impl Rendy {
    pub fn new(column_position: usize, color_code: ColorCode, buffer: &'static mut Buffer) -> Self {
        Self {
            column_position,
            color_code,
            buffer,
        }
    }

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
                self.buffer.chars[row][col] = ScreenChar {
                    character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {}
}
