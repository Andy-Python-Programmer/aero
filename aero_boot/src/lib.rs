#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[repr(C)]
pub struct FrameBufferInfo {
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub stride: usize,
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
