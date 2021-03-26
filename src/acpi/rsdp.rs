/// The RSDP (Root System Description Pointer)'s signature.
///
/// **Note**: The trailing space is required.
const RSDP_SIGNATURE: &[u8] = b"RSD PTR ";

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
    pub fn lookup(start_addr: usize, end_addr: usize) -> Option<Self> {
        for i in 0..(end_addr + 1 - start_addr) / 16 {
            let rsdp = unsafe { &*((start_addr + i * 16) as *const RSDP) };

            if &rsdp.signature == RSDP_SIGNATURE {
                return Some(*rsdp);
            }
        }

        None
    }

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
