/*
 * Copyright (C) 2021 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

use core::fmt::Write;

use core::fmt;
use core::u8;

use font8x8::UnicodeFonts;

use spin::{mutex::Mutex, MutexGuard, Once};
use stivale_boot::v2::StivaleFramebufferTag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Color(u32);

impl Color {
    pub const WHITE: Self = Self::from_hex(0xFFFFFF);
    pub const BLACK: Self = Self::from_hex(0x000000);

    #[inline(always)]
    pub const fn from_hex(hex: u32) -> Self {
        Self(hex)
    }

    #[inline(always)]
    pub const fn inner(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorCode(Color, Color);

impl ColorCode {
    #[inline(always)]
    pub fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode(foreground, background)
    }

    #[inline(always)]
    pub fn get_foreground(&self) -> Color {
        self.0
    }

    #[inline(always)]
    pub fn get_background(&self) -> Color {
        self.1
    }
}

/// Color format of pixels in the framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
#[allow(warnings)] // FIXME: Construct the other variants of this enum.
pub enum PixelFormat {
    /// One byte red, then one byte green, then one byte blue.
    RGB,
    /// One byte blue, then one byte green, then one byte red.
    BGR,
    /// A single byte, representing the grayscale value.
    U8,
}

/// Describes the layout and pixel format of a framebuffer.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FrameBufferInfo {
    /// The total size in bytes.
    pub byte_len: usize,
    /// The width in pixels.
    pub horizontal_resolution: usize,
    /// The height in pixels.
    pub vertical_resolution: usize,
    /// The color format of each pixel.
    pub pixel_format: PixelFormat,
    /// The number of bytes per pixel.
    pub bytes_per_pixel: usize,
    /// Number of pixels between the start of a line and the start of the next.
    ///
    /// Some framebuffers use additional padding at the end of a line, so this
    /// value might be larger than `horizontal_resolution`. It is
    /// therefore recommended to use this field for calculating the start address of a line.
    pub stride: usize,
}

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

        unsafe {
            core::slice::from_raw_parts_mut(self.buffer as *mut u8, self.info.byte_len).fill(0x00);
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

static DEBUG_RENDY: Once<Mutex<DebugRendy>> = Once::new();

pub macro print {
    ($($arg:tt)*) => ($crate::rendy::_print(format_args!($($arg)*))),
}

pub macro println {
    () => ($crate::rendy::print!("\n")),
    ($($arg:tt)*) => ($crate::rendy::print!("{}\n", format_args!($($arg)*))),
}

pub macro dbg {
    () => {
        $crate::rendy::println!("[{}:{}]", $crate::file!(), $crate::line!());
    },

    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::rendy::println!("[{}:{}] {} = {:#?}",
                    core::file!(), core::line!(), core::stringify!($val), &tmp);
                tmp
            }
        }
    },

    ($($val:expr),+ $(,)?) => {
        ($($crate::rendy::dbg!($val)),+,)
    },
}

/// Get a mutable reference to the debug renderer.
fn get_debug_rendy() -> MutexGuard<'static, DebugRendy> {
    DEBUG_RENDY
        .get()
        .expect("Attempted to get the debug renderer before it was initialized")
        .lock()
}

/// Return [true] if the debug renderer is initialized.
#[inline]
pub fn is_initialized() -> bool {
    DEBUG_RENDY.get().is_some()
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    get_debug_rendy()
        .write_fmt(args)
        .expect("Failed to write to the framebuffer");
}

pub fn set_color_code(color_code: ColorCode) {
    get_debug_rendy().set_color_code(color_code);
}

pub fn clear_screen() {
    get_debug_rendy().clear_screen();
}

/// Force-unlocks the rendy to prevent a deadlock.
///
/// ## Saftey
/// This method is not memory safe and should be only used when absolutely necessary.
pub unsafe fn force_unlock() {
    DEBUG_RENDY.get().map(|l| l.force_unlock());
}

pub fn init(framebuffer_tag: &'static StivaleFramebufferTag) {
    let framebuffer_info = FrameBufferInfo {
        byte_len: framebuffer_tag.size(),
        bytes_per_pixel: framebuffer_tag.framebuffer_bpp as usize,
        horizontal_resolution: framebuffer_tag.framebuffer_width as usize,
        vertical_resolution: framebuffer_tag.framebuffer_height as usize,
        pixel_format: PixelFormat::BGR,
        stride: framebuffer_tag.framebuffer_pitch as usize,
    };

    let mut rendy = DebugRendy::new(
        unsafe { crate::PHYSICAL_MEMORY_OFFSET + framebuffer_tag.framebuffer_addr }.as_u64(),
        framebuffer_info,
    );

    rendy.clear_screen();

    DEBUG_RENDY.call_once(|| Mutex::new(rendy));
}
