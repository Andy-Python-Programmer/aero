/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use core::ptr;

use x86_64::structures::paging::OffsetPageTable;

use super::sdt::Sdt;
use super::GenericAddressStructure;

pub const SIGNATURE: &str = "HPET";

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct Hpet {
    pub header: Sdt,
    pub hw_rev_id: u8,
    pub comparator_descriptor: u8,
    pub pci_vendor_id: u16,
    pub base_address: GenericAddressStructure,
    pub hpet_number: u8,
    pub min_periodic_clk_tick: u16,
    pub oem_attribute: u8,
}

impl Hpet {
    pub fn new(sdt: Option<&'static Sdt>, offset_table: &mut OffsetPageTable) -> Self {
        let sdt = sdt.expect("HPET not found");

        let this = unsafe { ptr::read((sdt as *const Sdt) as *const Self) };

        unsafe {
            this.base_address.init(offset_table);
        }

        this
    }
}
