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

use core::mem;

use crate::mem::paging::VirtAddr;

#[repr(C, packed)]
pub(super) struct Sdt {
    pub(super) signature: [u8; 4],
    pub(super) length: u32,
    pub(super) revision: u8,
    pub(super) checksum: u8,
    pub(super) oem_id: [u8; 6],
    pub(super) oem_table_id: [u8; 8],
    pub(super) oem_revision: u32,
    pub(super) creator_id: u32,
    pub(super) creator_revision: u32,
}

impl Sdt {
    pub fn is_valid(&self) -> bool {
        unsafe {
            let sptr = self as *const _ as *const u8;
            let size = self.length as usize;

            super::rsdp::validate_checksum(sptr, size)
        }
    }

    pub fn signature(&self) -> &[u8] {
        &self.signature as &[u8]
    }

    /// Get SDT from its address.
    #[inline]
    pub unsafe fn from_address(address: VirtAddr) -> &'static Self {
        &*(address.as_mut_ptr::<Self>())
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
