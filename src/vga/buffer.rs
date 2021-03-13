use volatile::Volatile;

use super::color::ColorCode;

pub(crate) const BUFFER_HEIGHT: usize = 25;
pub(crate) const BUFFER_WIDTH: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub(crate) struct ScreenChar {
    pub(crate) character: u8,
    pub(crate) color_code: ColorCode,
}

#[repr(transparent)]
pub struct Buffer {
    pub(crate) chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
