/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
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

use crate::cmdline::CommandLine;
use crate::mem;
use crate::mem::paging::align_up;

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
pub const X_PAD: usize = 1;

const MARGIN_GRADIENT: usize = 4;
const DWORD_SIZE: usize = core::mem::size_of::<u32>();

const DEFAULT_TEXT_BACKGROUND: u32 = u32::MAX;
const DEFAULT_TEXT_FOREGROUND: u32 = 0xaaaaaa;

pub const DEFAULT_THEME_BACKGROUND: u32 = 0x50000000;

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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct RendyInfo {
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

#[repr(C, packed)]
struct BmpHeader {
    bf_signature: [u8; 2],
    bf_size: u32,
    reserved: u32,
    bf_offset: u32,

    bi_size: u32,
    bi_width: u32,
    bi_height: u32,
    bi_planes: u16,
    bi_bpp: u16,
    bi_compression: u32,
    bi_image_size: u32,
    bi_xcount: u32,
    bi_ycount: u32,
    bi_clr_used: u32,
    bi_clr_important: u32,
    red_mask: u32,
    green_mask: u32,
    blue_mask: u32,
}

#[derive(Debug)]
struct Image {
    image: Box<[u8]>,

    img_width: usize,
    img_height: usize,
    bpp: usize,
    pitch: usize,
}

fn parse_bmp_image(data: &[u8]) -> Image {
    let header: &BmpHeader =
        unsafe { core::mem::transmute(data as *const [u8] as *const u8 as *const BmpHeader) };

    // Check if the BMP image has the correct signature (ie. "BM").
    assert!(&header.bf_signature == b"BM");

    // We do not support BPP lower then 8.
    assert!(header.bi_bpp % 8 == 0);

    let mut image = mem::alloc_boxed_buffer::<u8>(header.bf_size as usize);
    let bytes = image.len();

    (&mut image[..bytes - header.bf_offset as usize])
        .copy_from_slice(&data[header.bf_offset as usize..header.bf_size as usize]);

    Image {
        image,

        img_width: header.bi_width as usize,
        img_height: header.bi_height as usize,
        bpp: header.bi_bpp as usize,
        pitch: align_up((header.bi_width * header.bi_bpp as u32) as u64, 32) as usize / 8,
    }
}

pub struct DebugRendy<'this> {
    /// The raw framebuffer pointer queried from the BIOS or UEFI firmware represented
    /// as a [u8] slice.
    buffer: &'this mut [u32],
    info: RendyInfo,

    x_pos: usize,
    y_pos: usize,

    old_x_pos: usize,
    old_y_pos: usize,

    rows: usize,
    cols: usize,

    color: ColorCode,
    theme_background: u32,

    queue: Box<[QueueCharacter]>,
    grid: Box<[Character]>,
    map: Box<[Option<*mut QueueCharacter>]>,
    bg_canvas: Box<[u32]>,

    vga_font_bool: Box<[bool]>,

    queue_cursor: usize,

    glyph_width: usize,
    glyph_height: usize,

    offset_x: usize,
    offset_y: usize,

    cursor_visibility: bool,
    auto_flush: bool,
}

impl<'this> DebugRendy<'this> {
    /// Create a new debug renderer with the default foreground color set to white and
    /// background color set to black.
    pub fn new(buffer: &'this mut [u32], info: RendyInfo, cmdline: &CommandLine) -> Self {
        let width = info.horizontal_resolution;
        let height = info.vertical_resolution;

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

        let map_size = rows * cols * core::mem::size_of::<Option<*const QueueCharacter>>();
        let map = mem::alloc_boxed_buffer::<Option<*mut QueueCharacter>>(map_size);

        let bg_canvas_size = width * height * core::mem::size_of::<u32>();
        let bg_canvas = mem::alloc_boxed_buffer::<u32>(bg_canvas_size);

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

            theme_background: cmdline.theme_background,
            color: ColorCode::new(DEFAULT_TEXT_FOREGROUND, DEFAULT_TEXT_BACKGROUND),

            queue,
            grid,
            map,
            bg_canvas,

            glyph_height,
            glyph_width,

            offset_x,
            offset_y,

            vga_font_bool,

            queue_cursor: 0,

            cursor_visibility: true,
            auto_flush: true,
        };

        let image = cmdline.term_background.map(|a| parse_bmp_image(a));
        this.generate_canvas(image);

        this.clear(true);
        this.double_buffer_flush();

        this
    }

    fn genloop<F>(
        &mut self,
        image: &Image,
        xstart: usize,
        xend: usize,
        ystart: usize,
        yend: usize,
        blender: F,
    ) where
        F: Fn(usize, usize, u32) -> usize,
    {
        let img_width = image.img_width;
        let img_height = image.img_height;
        let img_pitch = image.pitch;

        let width = self.info.horizontal_resolution;
        let height = self.info.vertical_resolution;

        let colsize = image.bpp / 8;

        // Tiled Image:
        // for y in ystart..yend {
        //     let image_y = y % img_height;
        //     let mut image_x = xstart % img_width;
        //     let off = img_pitch * (img_height - 1 - image_y);
        //     let fb_off = self.info.stride / 4 * y;

        //     for x in xstart..xend {
        //         let img_pixel =
        //             unsafe { (image.image.as_ptr()).add(image_x * colsize + off) as *const u32 };

        //         let i = blender(x, y, unsafe { *img_pixel });

        //         unsafe {
        //             *self.buffer.as_mut_ptr().add(fb_off + x) = i as u32;
        //         }

        //         if image_x == img_width {
        //             image_x = 0;
        //         }

        //         image_x += 1;
        //     }
        // }

        // Stretched Image:
        //
        // So you can set x = xstart * ratio, and increment by ratio at each iteration.
        let int_to_fixedp6 = |v| v * 64;
        let fixedp6_to_int = |v| v / 64;

        for y in ystart..yend {
            let img_y = (y * img_height) / height; // Calculate Y with full precision :)
            let off = img_pitch * (img_height - 1 - img_y);

            let fb_off = self.info.stride / 4 * y;
            let canvas_off = width * y;

            let ratio = int_to_fixedp6(img_width) / width;
            let mut img_x = ratio * xstart;

            for x in xstart..xend {
                let img_pixel = unsafe {
                    (image.image.as_ptr()).add(fixedp6_to_int(img_x) * colsize + off) as *const u32
                };

                let i = blender(x, y, unsafe { *img_pixel });

                unsafe {
                    *self.buffer.as_mut_ptr().add(fb_off + x) = i as u32;
                    self.bg_canvas[canvas_off + x] = i as u32;
                }

                img_x += ratio;
            }
        }
    }

    pub fn get_framebuffer<'a>(&'a mut self) -> &'a mut [u32] {
        self.buffer
    }

    fn loop_external(
        &mut self,
        image: &Image,
        xstart: usize,
        xend: usize,
        ystart: usize,
        yend: usize,
    ) {
        self.genloop(image, xstart, xend, ystart, yend, |_, __, c| c as usize)
    }

    fn loop_internal(
        &mut self,
        image: &Image,
        xstart: usize,
        xend: usize,
        ystart: usize,
        yend: usize,
    ) {
        let color_blend = |fg: u32, bg: u32| {
            let alpha = 255 - (fg >> 24) as u8 as u32;
            let inv_alpha = (fg >> 24) as u8 as u32 + 1;

            let r = (alpha * (fg >> 16) as u8 as u32 + inv_alpha * (bg >> 16) as u8 as u32) / 256;
            let g = (alpha * (fg >> 8) as u8 as u32 + inv_alpha * (bg >> 8) as u8 as u32) / 256;
            let b = (alpha * fg as u8 as u32 + inv_alpha * bg as u8 as u32) / 256;

            (0 << 24) | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)
        };

        let theme_background = self.theme_background;

        self.genloop(image, xstart, xend, ystart, yend, |_, __, c| {
            let blend = color_blend(theme_background, c) as usize;
            blend
        })
    }

    fn generate_canvas(&mut self, image: Option<Image>) {
        let width = self.info.horizontal_resolution;
        let height = self.info.vertical_resolution;

        if let Some(image) = image {
            let frame_height = height / 2 - (self.glyph_height * self.rows) / 2;
            let frame_width = width / 2 - (self.glyph_width * self.cols) / 2;

            let frame_height_end = frame_height + self.glyph_height * self.rows;
            let frame_width_end = frame_width + self.glyph_width * self.cols;

            let fheight = frame_height - MARGIN_GRADIENT;
            let fheight_end = frame_height_end + MARGIN_GRADIENT;
            let fwidth = frame_width - MARGIN_GRADIENT;
            let fwidth_end = frame_width_end + MARGIN_GRADIENT;

            self.loop_external(&image, 0, width, 0, fheight);
            self.loop_external(&image, 0, width, fheight_end, height);
            self.loop_external(&image, 0, fwidth, fheight, fheight_end);
            self.loop_external(&image, fwidth_end, width, fheight, fheight_end);

            self.loop_internal(
                &image,
                frame_width,
                frame_width_end,
                frame_height,
                frame_height_end,
            );
        } else {
            for y in 0..height {
                for x in 0..width {
                    self.bg_canvas[y * width + x] = self.theme_background;
                    self.plot_pixel(x, y, self.theme_background);
                }
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

    fn clear(&mut self, mv: bool) {
        let char = Character {
            char: ' ',
            fg: self.color.get_foreground(),
            bg: self.color.get_background(),
        };

        for i in 0..self.rows * self.cols {
            self.push_to_queue(&char, i % self.cols, i / self.cols);
        }

        if mv {
            self.x_pos = X_PAD;
            self.y_pos = 0;
        }
    }

    fn set_auto_flush(&mut self, yes: bool) {
        self.auto_flush = yes;
    }

    fn write_string(&mut self, string: &str) {
        for char in string.chars() {
            self.write_character(char)
        }

        if self.auto_flush {
            self.double_buffer_flush();
        }
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

        // naming: fx, fy for font coordinates and gx, gy for glyph coordinates
        for gy in 0..self.glyph_height {
            let fb_line = unsafe {
                self.buffer
                    .as_mut_ptr()
                    .add(x + (y + gy) * (self.info.stride / 4))
            };

            let canvas_line = unsafe {
                self.bg_canvas
                    .as_mut_ptr()
                    .add(x + (y + gy) * self.info.horizontal_resolution)
            };

            for fx in 0..DEFAULT_FONT_WIDTH {
                let draw = unsafe { *glyph.add(gy * DEFAULT_FONT_WIDTH + fx) };

                let bg = if char.bg == u32::MAX {
                    unsafe { *canvas_line.add(fx) }
                } else {
                    char.bg
                };

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
        if self.cursor_visibility {
            self.draw_cursor();
        }

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

    fn backspace(&mut self) {
        let empty = Character {
            char: ' ',
            fg: self.color.get_foreground(),
            bg: self.color.get_background(),
        };

        if self.x_pos == 0 {
            self.y_pos -= 1;
            self.x_pos = self.cols - 1;
        } else {
            self.x_pos -= 1;
        }

        self.push_to_queue(&empty, self.x_pos, self.y_pos);
        self.double_buffer_flush();
    }

    fn set_cursor_position(&mut self, x: usize, y: usize) {
        assert!(x <= self.cols && y <= self.rows);

        self.x_pos = X_PAD + x;
        self.y_pos = y;
        self.double_buffer_flush();
    }
}

impl<'this> fmt::Write for DebugRendy<'this> {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_string(string);

        Ok(())
    }
}

unsafe impl<'this> Send for DebugRendy<'this> {}
unsafe impl<'this> Sync for DebugRendy<'this> {}

pub static DEBUG_RENDY: Once<Mutex<DebugRendy>> = Once::new();

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

/// Return true if the terminal is initialized.
pub fn is_initialized() -> bool {
    DEBUG_RENDY.get().is_some()
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    DEBUG_RENDY.get().map(|l| l.lock_irq().write_fmt(args));
}

/// Clears the screen and if `mv` is set to true, resets the
/// cursor position to `0`.
pub fn clear_screen(mv: bool) {
    DEBUG_RENDY.get().map(|l| l.lock_irq().clear(mv));
}

pub fn backspace() {
    DEBUG_RENDY.get().map(|l| l.lock_irq().backspace());
}

pub fn set_text_color(fg: u32, bg: u32) {
    DEBUG_RENDY
        .get()
        .map(|l| l.lock_irq().color = ColorCode::new(fg, bg));
}

pub fn set_text_fg(fg: u32) {
    DEBUG_RENDY.get().map(|l| l.lock_irq().color.0 = fg);
}

pub fn set_text_bg(bg: u32) {
    DEBUG_RENDY.get().map(|l| l.lock_irq().color.1 = bg);
}

/// Resets the text foreground and background to their default values.
pub fn reset_default() {
    set_text_color(DEFAULT_TEXT_FOREGROUND, DEFAULT_TEXT_BACKGROUND)
}

/// Returns the terminal's resolution in the form of a `(horizontal_resolution, vertical_resolution)`
/// tuple.
///
/// # Panics
/// Attempted to get the resolution before the terminal was initialized.
pub fn get_resolution() -> (usize, usize) {
    DEBUG_RENDY
        .get()
        .map(|l| {
            let this = l.lock_irq();

            (
                this.info.horizontal_resolution,
                this.info.vertical_resolution,
            )
        })
        .expect(
            "get_resolution: attempted to get the resolution before the terminal was initialized",
        )
}

/// Returns the terminal's rows and columns in the form of a `(rows, columns)` tuple.
pub fn get_rows_cols() -> (usize, usize) {
    DEBUG_RENDY
        .get()
        .map(|l| {
            let this = l.lock_irq();
            (this.rows, this.cols)
        })
        .expect("get_rows_cols: attempted to get the rows,cols before the terminal was initialized")
}

/// Gets the cursor position as a tuple `(x, y)`.
///
/// ## Notes
/// The return'ed cursor position will not have `X` and `Y` padding applied.
///
/// ## Panics
/// Attempted to get the cursor position before the terminal was initialized.
pub fn get_cursor_position() -> (usize, usize) {
    DEBUG_RENDY.get().map(|l| {
        let lock = l.lock_irq();
        (lock.x_pos-X_PAD, lock.y_pos)
    }).expect("get_cursor_position: attepted to get the cursor position before the terminal was initialized")
}

/// Sets the cursor position to the provided `x` and `y` coordinates.
///
/// ## Panics
/// * If the provided `x` position is greator then the amount of columns.
/// * If the provided `y` position is greator then the amount of rows.
pub fn set_cursor_position(x: usize, y: usize) {
    DEBUG_RENDY
        .get()
        .map(|l| l.lock_irq().set_cursor_position(x, y));
}

pub fn set_cursor_visibility(yes: bool) {
    DEBUG_RENDY
        .get()
        .map(|l| l.lock_irq().cursor_visibility = yes);
}

/// Returns a tuple of the amount of `(rows, columns)` in the terminal.
///
/// ## Panics
/// This function was called before the terminal was initialized.
pub fn get_term_info() -> (usize, usize) {
    DEBUG_RENDY.get().map(|l| {
        let lock = l.lock_irq();
        (lock.rows, lock.cols)
    }).expect("get_term_info: attepted to get the terminal information before the terminal was initialized")
}

/// ## Panics
/// This function was called before the terminal was initialized.
pub fn set_auto_flush(yes: bool) {
    DEBUG_RENDY
        .get()
        .map(|e| e.lock_irq().set_auto_flush(yes))
        .expect("set_auto_flush: attempted to set auto flush before the terminal was initialized");
}

/// ## Panics
/// This function was called before the terminal was initialized.
pub fn double_buffer_flush() {
    DEBUG_RENDY
        .get()
        .map(|e| e.lock_irq().double_buffer_flush())
        .expect("double_buffer_flush: attempted to flush before the terminal was initialized");
}

/// Force-unlocks the rendy to prevent a deadlock.
///
/// ## Saftey
/// This method is not memory safe and should be only used when absolutely necessary.
pub unsafe fn force_unlock() {
    DEBUG_RENDY.get().map(|l| l.force_unlock());
}

pub fn init(framebuffer_tag: &'static StivaleFramebufferTag, cmdline: &CommandLine) {
    let framebuffer_info = RendyInfo {
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

    let rendy = DebugRendy::new(framebuffer, framebuffer_info, cmdline);

    DEBUG_RENDY.call_once(|| Mutex::new(rendy));
}
