use core::mem;

/// The RSDP (Root System Description Pointer)'s signature.
///
/// **Note**: The trailing space is required.
const RSDP_SIGNATURE: &[u8] = b"RSD PTR ";

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct RSDP {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

impl RSDP {
    pub fn lookup(start_addr: usize, end_addr: usize) -> Option<Self> {
        for i in 0..(end_addr + 1 - start_addr) / mem::size_of::<RSDP>() {
            let rsdp = unsafe { &*(start_addr as *const RSDP).offset(i as isize) };

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
