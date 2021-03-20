const PAGE_SIZE: usize = 4096;

global_asm!(include_str!("enable_paging.s"));

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    /// Create a new virtual address.
    #[inline]
    pub fn new(address: u64) -> Self {
        Self(address)
    }

    /// Get the inner address.
    #[inline]
    pub fn address(&self) -> u64 {
        self.0
    }
}

/// Initialize paging.
pub fn init() {
    unsafe {
        EnablePaging();
    }
}

extern "C" {
    fn EnablePaging();
}
