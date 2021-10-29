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

use alloc::boxed::Box;

use spin::Once;

use crate::mem;

use stivale_boot::v2::StivaleFramebufferTag;

use crate::utils::sync::Mutex;

static FONT: &[u8] = include_bytes!("../../font.bin");

// This is an example of how the rendered screen will look like:
//
// ```text
// -----------------------------------------------------|
//                         YPAD                         |
// -----------------------------------------------------|
//  MARGIN | XPAD |        DATA         | XPAD | MARGIN |
// -----------------------------------------------------|
//  MARGIN | XPAD |        DATA         | XPAD | MARGIN |
// -----------------------------------------------------|
//                         YPAD                         |
// -----------------------------------------------------|
// ```

const DEFAULT_FONT_WIDTH: usize = 8;
const DEFAULT_FONT_HEIGHT: usize = 16;

const DEFAULT_MARGIN: usize = 64 / 2;

/// The amount of VGA font glyphs.
const VGA_FONT_GLYPHS: usize = 256;

/// Constant describing the number of columns padded at the left
/// and right of the screen.
const X_PAD: usize = 1;

const DEFAULT_BACKGROUND: u32 = u32::MAX;
const DWORD_SIZE: usize = core::mem::size_of::<u32>();

#[derive(Debug, Copy, Clone, PartialEq)]
struct Character {
    char: char,
    fg: u32,
    bg: u32,
}

#[derive(Debug, PartialEq, Clone)]
struct QueueCharacter {
    char: Character,
    x: usize,
    y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorCode(u32, u32);

impl ColorCode {
    pub fn new(foreground: u32, background: u32) -> ColorCode {
        ColorCode(foreground, background)
    }

    pub fn get_foreground(&self) -> u32 {
        self.0
    }

    pub fn get_background(&self) -> u32 {
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

pub struct DebugRendy {
    /// The raw framebuffer pointer queried from the BIOS or UEFI firmware represented
    /// as a [u8] slice.
    buffer: &'static mut [u32],
    info: FrameBufferInfo,

    x_pos: usize,
    y_pos: usize,

    old_x_pos: usize,
    old_y_pos: usize,

    rows: usize,
    cols: usize,

    color: ColorCode,

    queue: Box<[QueueCharacter]>,
    grid: Box<[Character]>,
    map: Box<[Option<*mut QueueCharacter>]>,

    vga_font_bool: Box<[bool]>,

    queue_cursor: usize,

    glyph_width: usize,
    glyph_height: usize,

    offset_x: usize,
    offset_y: usize,
}

impl DebugRendy {
    /// Create a new debug renderer with the default foreground color set to white and
    /// background color set to black.
    pub fn new(buffer: &'static mut [u32], info: FrameBufferInfo) -> Self {
        let glyph_width = DEFAULT_FONT_WIDTH;
        let glyph_height = DEFAULT_FONT_HEIGHT;

        let offset_x =
            DEFAULT_MARGIN + ((info.horizontal_resolution - DEFAULT_MARGIN * 2) % glyph_width) / 2;

        let offset_y =
            DEFAULT_MARGIN + ((info.horizontal_resolution - DEFAULT_MARGIN * 2) % glyph_height) / 2;

        let cols = (info.horizontal_resolution - DEFAULT_MARGIN * 2) / glyph_width;
        let rows = (info.vertical_resolution - DEFAULT_MARGIN * 2) / glyph_height;

        let grid_size = rows * cols * core::mem::size_of::<Character>();
        let grid = mem::alloc_boxed_buffer::<Character>(grid_size);

        let queue_size = rows * cols * core::mem::size_of::<QueueCharacter>();
        let queue = mem::alloc_boxed_buffer::<QueueCharacter>(queue_size);

        let map_size = rows * cols * core::mem::size_of::<*const QueueCharacter>();
        let map = mem::alloc_boxed_buffer::<Option<*mut QueueCharacter>>(map_size);

        let vga_font_bool_size = VGA_FONT_GLYPHS
            * DEFAULT_FONT_HEIGHT
            * DEFAULT_FONT_WIDTH
            * core::mem::size_of::<bool>();

        let mut vga_font_bool = mem::alloc_boxed_buffer::<bool>(vga_font_bool_size);

        for i in 0..VGA_FONT_GLYPHS {
            // Each glyph is a bitmap:
            let glyph = &FONT[i * DEFAULT_FONT_HEIGHT] as *const u8;

            for y in 0..DEFAULT_FONT_HEIGHT {
                // NOTE: the characters in VGA fonts are always one byte wide.
                // 9 dot wide fonts have 8 dots and one empty column, except
                // characters 0xC0-0xDF replicate column 9.
                for x in 0..8 {
                    let offset =
                        i * DEFAULT_FONT_HEIGHT * DEFAULT_FONT_WIDTH + y * DEFAULT_FONT_WIDTH + x;

                    unsafe {
                        if (*glyph.offset(y as isize) & (0x80 >> x)) != 0 {
                            vga_font_bool[offset] = true;
                        } else {
                            vga_font_bool[offset] = false;
                        }
                    }
                }

                // Fill columns above 8 like VGA Line Graphics Mode does:
                for x in 8..DEFAULT_FONT_WIDTH {
                    let offset =
                        i * DEFAULT_FONT_HEIGHT * DEFAULT_FONT_WIDTH + y * DEFAULT_FONT_WIDTH + x;

                    if i >= 0xC0 && i <= 0xDF {
                        unsafe {
                            vga_font_bool[offset] = (*glyph.offset(y as isize) & 1) != 0;
                        }
                    } else {
                        vga_font_bool[offset] = false;
                    }
                }
            }
        }

        let mut this = Self {
            buffer,
            info,

            x_pos: 0,
            y_pos: 0,

            old_x_pos: 0,
            old_y_pos: 0,

            rows,
            cols,

            color: ColorCode::new(u32::MAX, u32::MIN),

            queue,
            grid,
            map,

            glyph_height,
            glyph_width,

            offset_x,
            offset_y,

            vga_font_bool,

            queue_cursor: 0,
        };

        this.generate_canvas();

        this.clear();
        this.double_buffer_flush();

        this
    }

    fn generate_canvas(&mut self) {
        let width = self.info.horizontal_resolution;
        let height = self.info.vertical_resolution;

        for y in 0..height {
            for x in 0..width {
                self.plot_pixel(x, y, DEFAULT_BACKGROUND);
            }
        }
    }

    /// Plots a pixel at the given coordinates with the provided colour.
    fn plot_pixel(&mut self, x: usize, y: usize, colour: u32) {
        if x >= self.info.horizontal_resolution || y >= self.info.vertical_resolution {
            return;
        }

        let offset = x + (self.info.stride / DWORD_SIZE) * y;
        self.buffer[offset] = colour;
    }

    fn push_to_queue(&mut self, char: &Character, x: usize, y: usize) {
        if x >= self.cols || y >= self.rows {
            return;
        }

        let i = y * self.cols + x;
        let item = self.map[i];

        if item.is_none() {
            if &self.grid[i] == char {
                return;
            }

            let queue = &mut self.queue[self.queue_cursor];
            self.queue_cursor += 1;

            queue.x = x;
            queue.y = y;

            self.map[i] = Some(queue as *mut _);
        }

        let item = self.map[i];

        unsafe {
            (&mut *item.unwrap()).char = *char;
        }
    }

    fn clear(&mut self) {
        let char = Character {
            char: ' ',
            fg: self.color.get_foreground(),
            bg: self.color.get_background(),
        };

        for i in 0..self.rows * self.cols {
            self.push_to_queue(&char, i % self.cols, i / self.cols);
        }

        self.x_pos = X_PAD;
        self.y_pos = 0;
    }

    fn write_string(&mut self, string: &str) {
        for char in string.chars() {
            self.write_character(char)
        }

        self.double_buffer_flush();
    }

    fn draw_cursor(&mut self) {
        let i = self.x_pos + self.y_pos * self.cols;
        let mut char;

        if self.map[i].is_some() {
            unsafe {
                char = (&mut *self.map[i].unwrap()).char;
            }
        } else {
            char = self.grid[i];
        }

        let temp = char.fg;
        char.fg = char.bg;
        char.bg = temp;

        self.plot_char(self.x_pos, self.y_pos, char);

        if self.map[i].is_some() {
            unsafe {
                self.grid[i] = (&mut *self.map[i].unwrap()).char;
            }

            self.map[i] = None;
        }
    }

    fn plot_char(&mut self, x: usize, y: usize, char: Character) {
        if x >= self.cols || y >= self.rows {
            return;
        }

        let x = self.offset_x + x * self.glyph_width;
        let y = self.offset_y + y * self.glyph_height;

        let glyph = unsafe {
            self.vga_font_bool
                .as_ptr()
                .add(char.char as usize * DEFAULT_FONT_HEIGHT * DEFAULT_FONT_WIDTH)
        };

        // naming: fx,fy for font coordinates, gx,gy for glyph coordinates
        for gy in 0..self.glyph_height {
            let fb_line = unsafe {
                self.buffer
                    .as_mut_ptr()
                    .add(x + (y + gy) * (self.info.stride / 4))
            };

            for fx in 0..DEFAULT_FONT_WIDTH {
                let draw = unsafe { *glyph.add(gy * DEFAULT_FONT_WIDTH + fx) };

                let bg = char.bg;
                let fg = char.fg;

                unsafe {
                    if draw {
                        *fb_line.add(fx) = fg;
                    } else {
                        *fb_line.add(fx) = bg;
                    }
                }
            }
        }
    }

    fn double_buffer_flush(&mut self) {
        self.draw_cursor();

        for i in 0..self.queue_cursor {
            let queue = self.queue[i].clone();
            let offset = queue.y * self.cols + queue.x;

            if self.map[offset].is_none() {
                continue;
            }

            self.plot_char(queue.x, queue.y, queue.char);

            self.grid[offset] = queue.char;
            self.map[offset] = None;
        }

        if self.old_x_pos != self.x_pos || self.old_y_pos != self.y_pos {
            self.plot_char(
                self.old_x_pos,
                self.old_y_pos,
                self.grid[self.old_x_pos + self.old_y_pos * self.cols],
            );
        }

        self.old_x_pos = self.x_pos;
        self.old_y_pos = self.y_pos;

        self.queue_cursor = 0;
    }

    fn raw_put_char(&mut self, char: char) {
        let char = Character {
            char,
            fg: self.color.get_foreground(),
            bg: self.color.get_background(),
        };

        self.push_to_queue(&char, self.x_pos, self.y_pos);
        self.x_pos += 1;

        if self.x_pos == self.cols - X_PAD {
            self.x_pos = X_PAD;
            self.y_pos += 1;
        }

        if self.y_pos == self.rows {
            self.x_pos = X_PAD;
            self.y_pos -= 1;
            self.scroll();
        }
    }

    fn newline(&mut self) {
        if self.y_pos == self.rows - 1 {
            self.x_pos = X_PAD;
            self.scroll();
        } else {
            self.y_pos += 1;
            self.x_pos = X_PAD;
        }
    }

    fn write_character(&mut self, char: char) {
        match char {
            '\n' => self.newline(),
            '\r' => {}

            _ => {
                self.raw_put_char(char);
            }
        }
    }

    fn scroll(&mut self) {
        for i in X_PAD * self.cols..self.rows * self.cols {
            let queue = self.map[i];
            let res;

            if let Some(char) = queue {
                unsafe {
                    res = (*char).char;
                }
            } else {
                res = self.grid[i];
            }

            self.push_to_queue(
                &res,
                (i - self.cols) % self.cols,
                (i - self.cols) / self.cols,
            );
        }

        // Clear the last line of the screen.
        let empty = Character {
            char: ' ',
            fg: self.color.get_foreground(),
            bg: self.color.get_background(),
        };

        for i in ((self.rows - 1) * self.cols)..self.rows * self.cols {
            self.push_to_queue(&empty, i % self.cols, i / self.cols);
        }
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

/// Return [true] if the debug renderer is initialized.
#[inline]
pub fn is_initialized() -> bool {
    DEBUG_RENDY.get().is_some()
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    DEBUG_RENDY.get().map(|l| l.lock_irq().write_fmt(args));
}

pub fn clear_screen() {
    DEBUG_RENDY.get().map(|l| l.lock_irq().clear());
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

    let framebuffer_addr =
        unsafe { crate::PHYSICAL_MEMORY_OFFSET + framebuffer_tag.framebuffer_addr };

    let framebuffer = unsafe {
        core::slice::from_raw_parts_mut::<u32>(
            framebuffer_addr.as_mut_ptr(),
            framebuffer_info.byte_len,
        )
    };

    let rendy = DebugRendy::new(framebuffer, framebuffer_info);

    DEBUG_RENDY.call_once(|| Mutex::new(rendy));
}
