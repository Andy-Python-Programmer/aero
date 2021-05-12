#![no_std]

pub mod debug;

/// Color format of pixels in the framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
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

/// A pixel-based framebuffer that controls the screen output.
#[derive(Debug)]
#[repr(C)]
pub struct FrameBuffer {
    pub buffer_start: u64,
    pub buffer_byte_len: usize,
    pub info: FrameBufferInfo,
}

impl FrameBuffer {
    /// Returns the raw bytes of the framebuffer as slice.
    #[inline(always)]
    pub fn buffer<'a>(&self) -> &'a [u8] {
        unsafe { self.create_buffer() }
    }

    /// Returns the raw bytes of the framebuffer as mutable slice.
    #[inline(always)]
    pub fn buffer_mut<'a>(&mut self) -> &'a mut [u8] {
        unsafe { self.create_buffer() }
    }

    #[inline(always)]
    unsafe fn create_buffer<'a>(&self) -> &'a mut [u8] {
        core::slice::from_raw_parts_mut(self.buffer_start as *mut u8, self.buffer_byte_len)
    }

    /// Returns layout and pixel format information of the framebuffer.
    #[inline(always)]
    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }
}
