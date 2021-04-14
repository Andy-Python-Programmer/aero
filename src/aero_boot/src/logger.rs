use core::fmt;
use core::fmt::Write;

use aero_boot::{FrameBufferInfo, PixelFormat};

use font8x8::UnicodeFonts;
use spin::{Mutex, Once};

/// The global logger instance protected by a mutex.
pub static LOGGER: Once<LockedLogger> = Once::new();

pub struct LockedLogger(Mutex<Logger>);

impl LockedLogger {
    #[inline(always)]
    pub fn new(inner: Logger) -> Self {
        Self(Mutex::new(inner))
    }
}

pub struct Logger {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

impl Logger {
    /// Create a new logger from the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut this = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
        };

        this.clear_screen();
        this.y_pos = 2;

        this
    }

    pub fn write_string(&mut self, string: &str) {
        for character in string.chars() {
            self.write_character(character);
        }
    }

    pub fn write_character(&mut self, character: char) {
        match character {
            '\n' => self.new_line(),
            '\r' => self.carriage_return(),
            _ => {
                if self.x_pos >= self.width() {
                    self.new_line();
                }

                if self.y_pos >= (self.height() - 8) {
                    self.clear_screen();
                }

                let bytes = font8x8::BASIC_FONTS
                    .get(character)
                    .expect("Character not found in basic font");

                self.write_bytes(bytes);
            }
        }
    }

    pub fn write_bytes(&mut self, bytes: [u8; 8]) {
        for (y, byte) in bytes.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                let alpha = if *byte & (1 << bit) == 0 { 0 } else { 255 };

                self.put_pixel(self.x_pos + x, self.y_pos + y, alpha);
            }
        }

        self.x_pos += 8;
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.info.stride + x;

        let color = match self.info.pixel_format {
            PixelFormat::RGB => [intensity, intensity, intensity / 2, 0],
            PixelFormat::BGR => [intensity / 2, intensity, intensity, 0],
            PixelFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
    }

    #[inline(always)]
    fn width(&self) -> usize {
        self.info.horizontal_resolution
    }

    #[inline(always)]
    fn height(&self) -> usize {
        self.info.vertical_resolution
    }

    #[inline(always)]
    fn carriage_return(&mut self) {
        self.x_pos = 0;
    }

    fn new_line(&mut self) {
        self.y_pos += 8;

        self.carriage_return();
    }

    fn clear_screen(&mut self) {
        self.x_pos = 0;
        self.y_pos = 0;

        self.framebuffer.fill(0);
    }
}

impl log::Log for LockedLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut this = self.0.lock();

        writeln!(this, "[{}]:     {}", record.level(), record.args())
            .expect("Failed to write to the framebuffer");
    }

    fn flush(&self) {}
}

unsafe impl Send for Logger {}
unsafe impl Sync for Logger {}

impl fmt::Write for Logger {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_string(string);

        Ok(())
    }
}
