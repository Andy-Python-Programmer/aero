use core::ptr;

use super::{sdt::SDT, GenericAddressStructure};

pub const SIGNATURE: &str = "HPET";

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct HPET {
    pub header: SDT,
    pub hw_rev_id: u8,
    pub comparator_descriptor: u8,
    pub pci_vendor_id: u16,
    pub base_address: GenericAddressStructure,
    pub hpet_number: u8,
    pub min_periodic_clk_tick: u16,
    pub oem_attribute: u8,
}

impl HPET {
    pub fn new(sdt: Option<&'static SDT>) -> Self {
        let sdt = sdt.expect("HPET not found");

        unsafe { ptr::read((sdt as *const SDT) as *const Self) }
    }
}
