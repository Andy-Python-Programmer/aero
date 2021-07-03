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

use core::mem;

pub(super) const XSDT_SIGNATURE: &[u8; 4] = b"XSDT";
pub(super) const RSDT_SIGNATURE: &[u8; 4] = b"RSDT";

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Sdt {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

impl Sdt {
    /// Get SDT from its address.
    #[inline]
    pub unsafe fn from_address(address: u64) -> &'static Self {
        &*(address as *const Self)
    }

    /// Get the address of this tables data.
    #[inline]
    pub fn data_address(&self) -> usize {
        self as *const _ as usize + mem::size_of::<Self>()
    }

    /// Get the length of this tables data.
    pub fn data_len(&self) -> usize {
        let total_size = self.length as usize;
        let header_size = mem::size_of::<Self>();

        if total_size >= header_size {
            total_size - header_size
        } else {
            0
        }
    }

    #[inline]
    pub(super) unsafe fn as_ptr<T>(&self) -> &'static T {
        &*(self as *const _ as *const T)
    }
}
