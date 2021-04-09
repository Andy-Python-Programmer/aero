#![no_std]

pub struct FrameBufferInfo {
    pub horizontal_resolution: usize,
    pub vertical_resolution: usize,
    pub stride: usize,
}
