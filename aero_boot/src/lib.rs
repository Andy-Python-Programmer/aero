#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

use core::mem;
use x86_64::{PhysAddr, VirtAddr};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FrameBufferInfo {
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub stride: usize,
    pub size: usize,
    pub address: PhysAddr,
    pub pixel_format: PixelFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum PixelFormat {
    RGB,
    BGR,
    BitMask,
    BltOnly,
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub memory_type: MemoryRegionType,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
#[repr(C)]
pub enum MemoryRegionType {
    Usable,
    Bootloader,
    UnknownUefi(u32),
    UnknownBios(u32),
}

#[repr(C)]
pub struct FrameBuffer {
    inner: VirtAddr,
    size: usize,
}

impl FrameBuffer {
    #[inline]
    pub fn new(inner: VirtAddr, size: usize) -> Self {
        Self { inner, size }
    }

    pub unsafe fn write_value<T>(&mut self, index: usize, value: T) {
        debug_assert!(
            index.saturating_add(mem::size_of::<T>()) <= self.size,
            "Frame buffer accessed out of bounds"
        );

        ((self.inner.as_mut_ptr::<u8>()).add(index) as *mut T).write_volatile(value)
    }
}

#[repr(C)]
pub struct BootInfo {
    pub frame_buffer_info: FrameBufferInfo,
    pub frame_buffer: FrameBuffer,
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}
