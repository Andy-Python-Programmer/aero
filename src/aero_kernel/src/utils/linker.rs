use x86_64::VirtAddr;

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

    #[inline(always)]
    pub fn virt_addr(&'static self) -> VirtAddr {
        unsafe { VirtAddr::new_unsafe(self.as_usize() as u64) }
    }
}

unsafe impl Sync for LinkerSymbol {}
unsafe impl Send for LinkerSymbol {}
