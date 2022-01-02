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

use core::marker::PhantomData;

use super::sdt::Sdt;
use crate::mem::paging::VirtAddr;

pub(super) fn validate_rsdt_checksum<T: RsdtHeader>(header: &'static T) -> bool {
    if header.signature() == b"RSD PTR " {
        let hptr = header as *const _ as *const u8;
        unsafe { validate_checksum(hptr, core::mem::size_of::<T>()) }
    } else {
        false
    }
}

pub(super) unsafe fn validate_checksum(ptr: *const u8, size: usize) -> bool {
    let mut sum: u8 = 0;

    for i in 0..size {
        sum = sum.wrapping_add(*(ptr.add(i)));
    }

    sum == 0
}

pub(super) trait RsdtTyp {
    fn as_usize(self) -> usize;
}

impl RsdtTyp for u32 {
    fn as_usize(self) -> usize {
        self as _
    }
}

impl RsdtTyp for u64 {
    fn as_usize(self) -> usize {
        self as _
    }
}

pub(super) trait RsdtHeader {
    fn signature(&self) -> &[u8];
}

#[repr(C, packed)]
pub(super) struct Rsdp10 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

impl RsdtHeader for Rsdp10 {
    fn signature(&self) -> &[u8] {
        &self.signature as &[u8]
    }
}

#[repr(C, packed)]
pub(super) struct Rsdp20 {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,

    // Revision 2 after this comment:
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

impl RsdtHeader for Rsdp20 {
    fn signature(&self) -> &[u8] {
        &self.signature as &[u8]
    }
}

#[repr(C, packed)]
pub(super) struct Rsdt<T: RsdtTyp + core::marker::Sized> {
    pub header: Sdt,
    _phantom: PhantomData<T>,
}

impl Rsdt<u32> {
    pub fn new(address: VirtAddr) -> &'static Self {
        let this = unsafe { &*(address.as_ptr() as *const Self) };

        let valid_checksum = this.header.is_valid();
        let valid_signature = this.header.signature() == b"RSDT";

        assert!(valid_checksum, "rsdp: failed to validate RSDT checksum");
        assert!(valid_signature, "rsdp: failed to validate RSDT signature");

        this
    }

    pub fn entries_count(&self) -> usize {
        (self.header.length as usize - core::mem::size_of::<Self>()) / core::mem::size_of::<u32>()
    }

    pub fn lookup_entry(&self, signature: &str) -> Option<&'static Sdt> {
        let header_data_address = self.header.data_address() as *const u32;

        for i in 0..self.entries_count() {
            let item_addr_phys = unsafe { *(header_data_address.add(i)) } as u64;
            let item_addr_virt = unsafe { crate::PHYSICAL_MEMORY_OFFSET + item_addr_phys };

            let item = unsafe { Sdt::from_address(item_addr_virt) };

            if item.signature == signature.as_bytes() {
                return Some(item);
            }
        }

        None
    }
}

impl Rsdt<u64> {
    pub fn new(address: VirtAddr) -> &'static Self {
        let this = unsafe { &*(address.as_ptr() as *const Self) };

        let valid_checksum = this.header.is_valid();
        let valid_signature = this.header.signature() == b"XSDT";

        assert!(valid_checksum, "rsdp: failed to validate XSDT checksum");
        assert!(valid_signature, "rsdp: failed to validate XSDT signature");

        this
    }

    pub fn entries_count(&self) -> usize {
        (self.header.length as usize - core::mem::size_of::<Self>()) / core::mem::size_of::<u64>()
    }

    pub fn lookup_entry(&self, signature: &str) -> Option<&'static Sdt> {
        let header_data_address = self.header.data_address() as *const u64;

        for i in 0..self.entries_count() {
            let item_addr_phys = unsafe { *(header_data_address.add(i)) };
            let item_addr_virt = unsafe { crate::PHYSICAL_MEMORY_OFFSET + item_addr_phys };

            let item = unsafe { Sdt::from_address(item_addr_virt) };

            if item.signature == signature.as_bytes() {
                return Some(item);
            }
        }

        None
    }
}

pub(super) enum RsdtAddress {
    Xsdt(VirtAddr),
    Rsdt(VirtAddr),
}

pub(super) fn find_rsdt_address(rsdp_address: VirtAddr) -> RsdtAddress {
    // Look for RSDP v2 header, and if it does not exists, look for RSDP v1 header.
    let v20 = unsafe { &*(rsdp_address.as_ptr() as *const Rsdp20) };
    let is_v20 = v20.revision >= 2 && v20.xsdt_address != 0;

    if is_v20 {
        let valid = validate_rsdt_checksum(v20);
        assert!(valid, "rsdp: failed to validate RSDP v20 checksum");

        let xsdt_address = unsafe { crate::PHYSICAL_MEMORY_OFFSET + v20.xsdt_address };
        return RsdtAddress::Xsdt(xsdt_address);
    } else {
        let v10 = unsafe { &*(rsdp_address.as_ptr() as *const Rsdp10) };
        let valid = validate_rsdt_checksum(v10);

        assert!(valid, "rsdp: failed to validate RSDP v10 checksum");

        let rsdt_address = unsafe { crate::PHYSICAL_MEMORY_OFFSET + v10.rsdt_address as u64 };
        return RsdtAddress::Rsdt(rsdt_address);
    }
}
