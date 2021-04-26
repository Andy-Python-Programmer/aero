#[derive(Copy, Clone)]
#[repr(C, packed)]
pub(super) struct Rsdp {
    pub(super) signature: [u8; 8],
    pub(super) checksum: u8,
    pub(super) oemid: [u8; 6],
    pub(super) revision: u8,
    pub(super) rsdt_address: u32,
    pub(super) length: u32,
    pub(super) xsdt_address: u64,
    pub(super) extended_checksum: u8,
    pub(super) reserved: [u8; 3],
}

impl Rsdp {
    /// Get the SDT address.
    ///
    /// Returns the RSDT address if the revision is `0` else it returns the XSDT address.
    #[inline]
    pub(super) fn get_sdt_address(&self) -> usize {
        if self.revision == 0 {
            self.rsdt_address as usize
        } else {
            self.xsdt_address as usize
        }
    }
}
