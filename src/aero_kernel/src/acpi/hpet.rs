/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

use core::ptr;

use super::sdt::Sdt;
use super::GenericAddressStructure;

pub const SIGNATURE: &str = "HPET";

#[repr(C, packed)]
pub(super) struct Hpet {
    header: Sdt,
    hw_rev_id: u8,
    comparator_descriptor: u8,
    pci_vendor_id: u16,
    base_address: GenericAddressStructure,
    hpet_number: u8,
    min_periodic_clk_tick: u16,
    oem_attribute: u8,
}

impl Hpet {
    pub fn new(sdt: &'static Sdt) -> Self {
        let this = unsafe { ptr::read((sdt as *const Sdt) as *const Self) };

        this
    }
}
