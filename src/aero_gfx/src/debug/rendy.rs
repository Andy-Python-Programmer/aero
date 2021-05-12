use core::{fmt, u8};

use font8x8::UnicodeFonts;

use super::color::{Color, ColorCode};
use crate::FrameBufferInfo;

/// Debug renderer used by the kernel and the bootloader to log messages to the
/// framebuffer queried from the BIOS or UEFI firmware.
pub struct DebugRendy {
    /// The raw framebuffer pointer queried from the BIOS or UEFI firmware represented
    /// as a [u8] slice.
    buffer: u64,
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    color: ColorCode,
}

impl DebugRendy {
    /// Create a new debug renderer with the default foreground color set to white and
    /// background color set to black.
    ///
    /// **Note**: The debug renderer should **not** be used after GUI has started. Use the
    /// respective VGA functions instead.
    #[inline]
    pub fn new(buffer: u64, info: FrameBufferInfo) -> Self {
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
        // SAFTEY: Safe as we are 100% sure the x, y will be correct.
        unsafe {
            *((self.buffer as usize + (x * (self.info.bytes_per_pixel / 8) + y * self.info.stride))
                as *mut u32) = color.inner();
        }
    }

    pub fn clear_screen(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;

        // SAFTEY: Safe as we are looping under the buffer byte len.
        unsafe {
            for i in 0..self.info.byte_len {
                *((self.buffer as *mut u8).add(i)) = self.color.get_background().inner() as u8;
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

    #[inline(always)]
    pub fn set_color_code(&mut self, color: ColorCode) {
        self.color = color;
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

impl fmt::Write for DebugRendy {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_string(string);

        Ok(())
    }
}

unsafe impl Send for DebugRendy {}
unsafe impl Sync for DebugRendy {}
