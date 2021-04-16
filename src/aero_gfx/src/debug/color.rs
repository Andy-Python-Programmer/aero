#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorCode(u32, u32);

impl ColorCode {
    #[inline(always)]
    pub fn new(foreground: u32, background: u32) -> ColorCode {
        ColorCode(foreground, background)
    }

    #[inline(always)]
    pub fn get_foreground(&self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub fn get_background(&self) -> u32 {
        self.1
    }
}
