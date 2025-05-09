// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use core::fmt::Write;

use core::fmt;
use core::ops::{Index, IndexMut};
use core::ptr::NonNull;
use core::time::Duration;

use alloc::boxed::Box;

use limine::framebuffer::Framebuffer;
use spin::Once;
use vte::ansi::{Handler, NamedColor, Timeout};

use crate::cmdline::CommandLine;
use crate::mem;
use crate::mem::paging::align_up;

use crate::utils::sync::Mutex;

use vte::ansi::{Attr, Processor};

static FONT: &[[u8; FONT_HEIGHT]; FONT_GLYPHS] =
    unsafe { &core::mem::transmute(*include_bytes!("../../font.bin")) };

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

const FONT_WIDTH: usize = 8;
const FONT_HEIGHT: usize = 16;
const FONT_GLYPHS: usize = 256;

const DEFAULT_MARGIN: usize = 64 / 2;
const TAB_SIZE: usize = 4;

/// The amount of VGA font glyphs.

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
    /// The number of bits per pixel.
    pub bits_per_pixel: usize,
    /// Number of pixels between the start of a line and the start of the next.
    ///
    /// Some framebuffers use additional padding at the end of a line, so this
    /// value might be larger than `horizontal_resolution`. It is
    /// therefore recommended to use this field for calculating the start address of a line.
    pub stride: usize,

    pub red_mask_shift: u8,
    pub red_mask_size: u8,

    pub green_mask_shift: u8,
    pub green_mask_size: u8,

    pub blue_mask_shift: u8,
    pub blue_mask_size: u8,
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
    let header: &BmpHeader = unsafe { &*(data.as_ptr().cast::<BmpHeader>()) };

    // Check if the BMP image has the correct signature (ie. "BM").
    assert!(&header.bf_signature == b"BM");

    // We do not support BPP lower then 8.
    assert!(header.bi_bpp % 8 == 0);

    let mut image = mem::alloc_boxed_buffer::<u8>(header.bf_size as usize);
    let bytes = image.len();

    image[..bytes - header.bf_offset as usize]
        .copy_from_slice(&data[header.bf_offset as usize..header.bf_size as usize]);

    Image {
        image,

        img_width: header.bi_width as usize,
        img_height: header.bi_height as usize,
        bpp: header.bi_bpp as usize,
        pitch: align_up((header.bi_width * header.bi_bpp as u32) as u64, 32) as usize / 8,
    }
}

#[derive(Default)]
struct RendySync;

impl Timeout for RendySync {
    fn set_timeout(&mut self, _duration: Duration) {
        unimplemented!()
    }

    fn clear_timeout(&mut self) {
        unimplemented!()
    }

    fn pending_timeout(&self) -> bool {
        false
    }
}

const COLOR_COUNT: usize = 269;

struct ColorList([u32; COLOR_COUNT]);

impl ColorList {
    fn new() -> Self {
        let mut list = Self([0; COLOR_COUNT]);

        // The color values are based from the default alacritty colors.
        //
        // Normal colors:
        list[NamedColor::Black] = 0x1d1f21;
        list[NamedColor::Red] = 0xcc6666;
        list[NamedColor::Green] = 0xb5bd68;
        list[NamedColor::Yellow] = 0xf0c674;
        list[NamedColor::Blue] = 0x81a2be;
        list[NamedColor::Magenta] = 0xb294bb;
        list[NamedColor::Cyan] = 0x8abeb7;
        list[NamedColor::White] = 0xc5c8c6;
        // Bright colors:
        list[NamedColor::BrightBlack] = 0x666666;
        list[NamedColor::BrightRed] = 0xd54e53;
        list[NamedColor::BrightGreen] = 0xb9ca4a;
        list[NamedColor::BrightYellow] = 0xe7c547;
        list[NamedColor::BrightBlue] = 0x7aa6da;
        list[NamedColor::BrightMagenta] = 0xc397d8;
        list[NamedColor::BrightCyan] = 0x70c0b1;
        list[NamedColor::BrightWhite] = 0xeaeaea;

        list
    }
}

impl Index<NamedColor> for ColorList {
    type Output = u32;

    #[inline]
    fn index(&self, idx: NamedColor) -> &Self::Output {
        &self.0[idx as usize]
    }
}

impl IndexMut<NamedColor> for ColorList {
    #[inline]
    fn index_mut(&mut self, idx: NamedColor) -> &mut Self::Output {
        &mut self.0[idx as usize]
    }
}

impl Index<usize> for ColorList {
    type Output = u32;

    #[inline]
    fn index(&self, idx: usize) -> &Self::Output {
        &self.0[idx]
    }
}

pub struct Inner<'this> {
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
    map: Box<[Option<NonNull<QueueCharacter>>]>,
    bg_canvas: Box<[u32]>,

    queue_cursor: usize,

    offset_x: usize,
    offset_y: usize,

    cursor_visibility: bool,
    auto_flush: bool,

    color_list: ColorList,
}

impl Inner<'_> {
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
        let col_size = image.bpp / 8;

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
                let offset = fixedp6_to_int(img_x) * col_size + off;
                let img_pixel: [u8; 4] = unsafe { *image.image.as_ptr().add(offset).cast() };
                let i = blender(x, y, u32::from_le_bytes(img_pixel));

                unsafe {
                    *self.buffer.as_mut_ptr().add(fb_off + x) = i as u32;
                    self.bg_canvas[canvas_off + x] = i as u32;
                }

                img_x += ratio;
            }
        }
    }

    pub fn get_framebuffer(&mut self) -> &mut [u32] {
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

            ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF)
        };

        let theme_background = self.theme_background;

        self.genloop(image, xstart, xend, ystart, yend, |_, __, color| {
            color_blend(theme_background, color) as usize
        });
    }

    fn generate_canvas(&mut self, image: Option<Image>) {
        let width = self.info.horizontal_resolution;
        let height = self.info.vertical_resolution;

        if let Some(image) = image {
            let frame_width = width / 2 - (FONT_WIDTH * self.cols) / 2;
            let frame_height = height / 2 - (FONT_HEIGHT * self.rows) / 2;

            let frame_width_end = frame_width + FONT_WIDTH * self.cols;
            let frame_height_end = frame_height + FONT_HEIGHT * self.rows;

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

            self.map[i] = Some(NonNull::from(queue));
        }

        let item = self.map[i];

        unsafe {
            item.unwrap().as_mut().char = *char;
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
            self.x_pos = 0;
            self.y_pos = 0;
        }
    }

    fn draw_cursor(&mut self) {
        let i = self.x_pos + self.y_pos * self.cols;
        let mut char;

        if self.map[i].is_some() {
            unsafe {
                char = self.map[i].unwrap().as_ref().char;
            }
        } else {
            char = self.grid[i];
        }

        core::mem::swap(&mut char.fg, &mut char.bg);

        self.plot_char(self.x_pos, self.y_pos, char);

        if self.map[i].is_some() {
            unsafe {
                self.grid[i] = self.map[i].unwrap().as_ref().char;
            }

            self.map[i] = None;
        }
    }

    fn plot_char(&mut self, x: usize, y: usize, char: Character) {
        let ch = match char.char {
            ch if ch.is_ascii() => ch,
            _ => '?',
        };

        if x >= self.cols || y >= self.rows {
            return;
        }

        let x = self.offset_x + x * FONT_WIDTH;
        let y = self.offset_y + y * FONT_HEIGHT;
        let glyph = &FONT[ch as usize];

        // naming: fx, fy for font coordinates and gx, gy for glyph coordinates
        for (gy, glyph) in glyph.iter().enumerate().take(FONT_HEIGHT) {
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

            for gx in 0..FONT_WIDTH {
                let draw = *glyph & (1 << (FONT_WIDTH - gx - 1)) != 0;
                let color = if draw {
                    char.fg
                } else if char.bg == u32::MAX {
                    unsafe { *canvas_line.add(gx) }
                } else {
                    char.bg
                };

                unsafe {
                    *fb_line.add(gx) = color;
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

        if self.x_pos == self.cols {
            self.x_pos = 0;
            self.y_pos += 1;
        }

        if self.y_pos == self.rows {
            self.x_pos = 0;
            self.y_pos -= 1;
            self.scroll();
        }
    }

    fn newline(&mut self) {
        if self.y_pos == self.rows - 1 {
            self.x_pos = 0;
            self.scroll();
        } else {
            self.y_pos += 1;
            self.x_pos = 0;
        }
    }

    fn write_character(&mut self, char: char) {
        match char {
            '\n' => self.newline(),

            '\t' => {
                if (self.x_pos / TAB_SIZE + 1) >= self.cols {
                    self.set_cursor_position(self.cols - 1, self.y_pos);
                    return;
                }

                self.set_cursor_position((self.x_pos / TAB_SIZE + 1) * TAB_SIZE, self.y_pos);
            }

            '\r' => {}

            _ => {
                self.raw_put_char(char);
            }
        }
    }

    fn scroll(&mut self) {
        for i in self.cols..self.rows * self.cols {
            let queue = self.map[i];
            let res;

            if let Some(char) = queue {
                unsafe {
                    res = char.as_ref().char;
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

    fn set_cursor_position(&mut self, x: usize, y: usize) {
        assert!(x <= self.cols && y <= self.rows);

        self.x_pos = x;
        self.y_pos = y;
        self.double_buffer_flush();
    }
}

pub struct DebugRendy<'a> {
    inner: Inner<'a>,
    performer: Processor<RendySync>,
}

impl<'this> DebugRendy<'this> {
    /// Create a new debug renderer with the default foreground color set to white and
    /// background color set to black.
    pub fn new(buffer: &'this mut [u32], info: RendyInfo, cmdline: &CommandLine) -> Self {
        let width = info.horizontal_resolution;
        let height = info.vertical_resolution;

        let offset_x = DEFAULT_MARGIN + ((width - DEFAULT_MARGIN * 2) % FONT_WIDTH) / 2;
        let offset_y = DEFAULT_MARGIN + ((height - DEFAULT_MARGIN * 2) % FONT_HEIGHT) / 2;

        let cols = (width - DEFAULT_MARGIN * 2) / FONT_WIDTH;
        let rows = (height - DEFAULT_MARGIN * 2) / FONT_HEIGHT;

        let grid = mem::alloc_boxed_buffer::<Character>(rows * cols);
        let queue = mem::alloc_boxed_buffer::<QueueCharacter>(rows * cols);
        let map = mem::alloc_boxed_buffer::<Option<NonNull<QueueCharacter>>>(rows * cols);
        let bg_canvas = mem::alloc_boxed_buffer::<u32>(width * height);

        let mut this = Self {
            inner: Inner {
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

                queue_cursor: 0,

                offset_x,
                offset_y,

                cursor_visibility: true,
                auto_flush: true,

                color_list: ColorList::new(),
            },
            performer: Processor::new(),
        };

        let image = cmdline.term_background.map(parse_bmp_image);

        this.generate_canvas(image);
        this.clear(true);
        this.double_buffer_flush();

        this
    }
}

impl<'a> core::ops::Deref for DebugRendy<'a> {
    type Target = Inner<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl core::ops::DerefMut for DebugRendy<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl fmt::Write for DebugRendy<'_> {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.performer.advance(&mut self.inner, string.as_bytes());
        Ok(())
    }
}

impl vte::ansi::Handler for Inner<'_> {
    fn input(&mut self, c: char) {
        self.write_character(c);

        if self.auto_flush {
            self.double_buffer_flush();
        }
    }

    #[inline]
    fn linefeed(&mut self) {
        self.input('\n');
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

    fn terminal_attribute(&mut self, attr: Attr) {
        match attr {
            Attr::Reset => {
                self.color = ColorCode::new(DEFAULT_TEXT_FOREGROUND, DEFAULT_TEXT_BACKGROUND)
            }

            // Attr::Bold => todo!(),
            // Attr::Dim => todo!(),
            // Attr::Italic => todo!(),
            // Attr::Underline => todo!(),
            // Attr::DoubleUnderline => todo!(),
            // Attr::Undercurl => todo!(),
            // Attr::DottedUnderline => todo!(),
            // Attr::DashedUnderline => todo!(),
            // Attr::BlinkSlow => todo!(),
            // Attr::BlinkFast => todo!(),
            // Attr::Reverse => todo!(),
            // Attr::Hidden => todo!(),
            // Attr::Strike => todo!(),
            // Attr::CancelBold => todo!(),
            // Attr::CancelBoldDim => todo!(),
            // Attr::CancelItalic => todo!(),
            // Attr::CancelUnderline => todo!(),
            // Attr::CancelBlink => todo!(),
            // Attr::CancelReverse => todo!(),
            // Attr::CancelHidden => todo!(),
            // Attr::CancelStrike => todo!(),
            Attr::Foreground(color) => {
                let code = match color {
                    vte::ansi::Color::Named(c) => self.color_list[c],
                    vte::ansi::Color::Indexed(c) => self.color_list[c as usize],
                    _ => unimplemented!(),
                };

                self.color.0 = code;
            }
            // Attr::Background(_) => todo!(),
            // Attr::UnderlineColor(_) => todo!(),
            _ => {}
        }
    }
}

unsafe impl Send for DebugRendy<'_> {}
unsafe impl Sync for DebugRendy<'_> {}

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
        log::debug!("[{}:{}]", $crate::file!(), $crate::line!());
    },

    ($val:expr $(,)?) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                log::debug!("[{}:{}] {} = {:#?}", core::file!(), core::line!(), core::stringify!($val), &tmp);
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
    if let Some(l) = DEBUG_RENDY.get() {
        l.lock_irq().clear(mv)
    }
}

pub fn backspace() {
    if let Some(l) = DEBUG_RENDY.get() {
        l.lock_irq().backspace()
    }
}

/// Returns the terminal's resolution in the form of a `(horizontal_resolution,
/// vertical_resolution)` tuple.
///
/// # Panics
/// This function was called before the terminal was initialized.
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
        .expect("get_resolution: invoked before the terminal was initialized")
}

/// # Panics
/// This function was called before the terminal was initialized.
pub fn get_rendy_info() -> RendyInfo {
    DEBUG_RENDY
        .get()
        .map(|l| {
            let this = l.lock_irq();
            this.info
        })
        .expect("get_rendy_info: invoked before the terminal was initialized")
}

/// Returns the terminal's rows and columns in the form of a `(rows, columns)` tuple.
pub fn get_rows_cols() -> (usize, usize) {
    DEBUG_RENDY
        .get()
        .map(|l| {
            let this = l.lock_irq();
            (this.rows, this.cols)
        })
        .expect("get_rows_cols: invoked before the terminal was initialized")
}

/// Gets the cursor position as a tuple `(x, y)`.
///
/// ## Panics
/// Attempted to get the cursor position before the terminal was initialized.
pub fn get_cursor_position() -> (usize, usize) {
    DEBUG_RENDY
        .get()
        .map(|l| {
            let lock = l.lock_irq();
            (lock.x_pos, lock.y_pos)
        })
        .expect("get_cursor_position: invoked before the terminal was initialized")
}

/// Sets the cursor position to the provided `x` and `y` coordinates.
///
/// ## Panics
/// * If the provided `x` position is greator then the amount of columns.
/// * If the provided `y` position is greator then the amount of rows.
pub fn set_cursor_position(x: usize, y: usize) {
    if let Some(l) = DEBUG_RENDY.get() {
        l.lock_irq().set_cursor_position(x, y)
    }
}

/// Force-unlocks the rendy to prevent a deadlock.
///
/// ## Safety
/// This method is not memory safe and should be only used when absolutely necessary.
pub unsafe fn force_unlock() {
    if let Some(l) = DEBUG_RENDY.get() {
        l.force_unlock()
    }
}

pub fn init(fb_info: Framebuffer, cmdline: &CommandLine) {
    let stride = fb_info.pitch() as usize;
    let height = fb_info.height() as usize;
    let bits_per_pixel = fb_info.bpp() as usize;
    let byte_len = stride * height * (bits_per_pixel / 8);

    let framebuffer_info = RendyInfo {
        byte_len,
        bits_per_pixel,
        horizontal_resolution: fb_info.width() as usize,
        vertical_resolution: height,
        pixel_format: PixelFormat::BGR,
        stride,

        red_mask_shift: fb_info.red_mask_shift(),
        red_mask_size: fb_info.red_mask_size(),

        green_mask_shift: fb_info.green_mask_shift(),
        green_mask_size: fb_info.green_mask_size(),

        blue_mask_shift: fb_info.blue_mask_shift(),
        blue_mask_size: fb_info.blue_mask_size(),
    };

    let framebuffer = unsafe {
        core::slice::from_raw_parts_mut::<u32>(
            fb_info.addr().cast::<u32>(),
            framebuffer_info.byte_len,
        )
    };

    let rendy = DebugRendy::new(framebuffer, framebuffer_info, cmdline);

    DEBUG_RENDY.call_once(|| Mutex::new(rendy));
}
