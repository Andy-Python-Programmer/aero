#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[repr(C)]
pub struct FrameBufferInfo {
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub stride: usize,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
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
pub struct BootInfo {
    pub frame_buffer_info: FrameBufferInfo,
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}
