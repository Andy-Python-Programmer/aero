extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    #[inline(always)]
    pub fn as_ptr(&'static self) -> *const u8 {
        self as *const Self as *const u8
    }

    #[inline(always)]
    pub fn as_usize(&'static self) -> usize {
        self.as_ptr() as usize
    }
}

unsafe impl Sync for LinkerSymbol {}
unsafe impl Send for LinkerSymbol {}
