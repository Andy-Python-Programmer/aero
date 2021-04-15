#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct RSDP {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oemid: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub reserved: [u8; 3],
}

impl RSDP {
    /// Get the SDT address.
    ///
    /// Returns the RSDT address if the revision is `0` else it returns the XSDT address.
    pub fn get_sdt_address(&self) -> usize {
        if self.revision == 0 {
            self.rsdt_address as usize
        } else {
            self.xsdt_address as usize
        }
    }
}
