use core::fmt;

use font8x8::UnicodeFonts;

use super::color::{Color, ColorCode};
use crate::FrameBufferInfo;

/// Debug renderer used by the kernel and the bootloader to log messages to the
/// framebuffer queried from the BIOS or UEFI firmware.
pub struct DebugRendy<'buffer> {
    /// The raw framebuffer pointer queried from the BIOS or UEFI firmware represented
    /// as a [u8] slice.
    buffer: &'buffer mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    color: ColorCode,
}

impl<'buffer> DebugRendy<'buffer> {
    /// Create a new debug renderer with the default foreground color set to white and
    /// background color set to black.
    ///
    /// **Note**: The debug renderer should **not** be used after GUI has started. Use the
    /// respective VGA functions instead.
    #[inline]
    pub fn new(buffer: &'buffer mut [u8], info: FrameBufferInfo) -> Self {
        Self {
            buffer,
            info,
            x_pos: 0,
            y_pos: 0,
            color: ColorCode::new(Color::WHITE, Color::BLACK),
        }
    }

    pub fn write_string(&mut self, string: &str) {
        for char in string.chars() {
            self.write_character(char)
        }
    }

    pub fn write_character(&mut self, char: char) {
        match char {
            '\n' => self.new_line(),
            '\r' => self.carriage_return(),
            _ => {
                let char = font8x8::BASIC_FONTS.get(char).unwrap();

                if self.x_pos >= self.width() {
                    self.new_line();
                }

                if self.y_pos >= (self.height() - 16) {
                    self.clear_screen()
                }

                self.put_bytes(&char);
            }
        }
    }

    pub fn put_bytes(&mut self, bytes: &[u8]) {
        for (y, byte) in bytes.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                let background = *byte & (1 << bit) == 0;

                if background {
                    self.put_pixel(self.x_pos + x, self.y_pos + y, self.color.get_background());
                } else {
                    self.put_pixel(self.x_pos + x, self.y_pos + y, self.color.get_foreground());
                }
            }
        }

        self.x_pos += 8;
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: Color) {
        let pixel_offset = y * self.info.stride + x;

        let color = [
            color.get_red_bit(),
            color.get_green_bit(),
            color.get_blue_bit(),
            color.get_alpha_bit(),
        ];

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        self.buffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
    }

    pub fn clear_screen(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;

        self.buffer.fill(self.color.get_background().inner() as u8);
    }

    fn new_line(&mut self) {
        self.y_pos += 16;

        self.carriage_return()
    }

    #[inline(always)]
    fn carriage_return(&mut self) {
        self.x_pos = 0;
    }

    #[inline(always)]
    pub fn set_color_code(&mut self, color: ColorCode) {
        // Do not set the color again if its the same color.
        if !(color == self.color) {
            self.color = color;
        }
    }

    #[inline(always)]
    pub fn width(&self) -> usize {
        self.info.horizontal_resolution
    }

    #[inline(always)]
    pub fn height(&self) -> usize {
        self.info.vertical_resolution
    }
}

impl<'buffer> fmt::Write for DebugRendy<'buffer> {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_string(string);

        Ok(())
    }
}

unsafe impl<'buffer> Send for DebugRendy<'buffer> {}
unsafe impl<'buffer> Sync for DebugRendy<'buffer> {}
