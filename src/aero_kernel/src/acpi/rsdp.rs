/*
 * Copyright (C) 2021 The Aero Project Developers.
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
