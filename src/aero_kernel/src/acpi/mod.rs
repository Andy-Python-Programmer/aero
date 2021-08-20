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

//! The ACPI (Advanced Configuration and Power Interface) tables help to gather the
//! CPU, interrupt, and timer informations.
//!
//! **Notes**: <https://wiki.osdev.org/ACPI>

use core::mem;

use aml::AmlContext;

use crate::mem::paging::{PhysAddr, VirtAddr};

use self::{fadt::Fadt, hpet::Hpet, madt::Madt, mcfg::Mcfg, rsdp::Rsdp, sdt::Sdt};

pub mod fadt;
pub mod hpet;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod sdt;

enum AcpiHeader {
    Rsdt(&'static Sdt),
    Xsdt(&'static Sdt),
}

impl AcpiHeader {
    fn as_sdt(&self) -> &'static Sdt {
        match self {
            AcpiHeader::Rsdt(rsdt) => rsdt,
            AcpiHeader::Xsdt(xsdt) => xsdt,
        }
    }

    /// The data address of this header's data.
    fn data_address(&self) -> usize {
        self.as_sdt().data_address()
    }
}

pub struct AcpiTable {
    header: AcpiHeader,
    entry_count: usize,
}

impl AcpiTable {
    /// Create a new ACPI table from the RSDP address.
    fn new(rsdp_address: VirtAddr) -> Self {
        // SAFTEY: Safe to cast the RSDP address to the RSDP struct as the
        // address is verified by the bootloader.
        let rsdp = unsafe { &*(rsdp_address.as_u64() as *const Rsdp) };
        let sdt_address = rsdp.get_sdt_address() as u64;

        // SAFTEY: Already would have caused UB if the RSDP address was
        // anyhow invalid.
        let sdt = unsafe { Sdt::from_address(sdt_address) };

        let sdt_signature = &sdt.signature;
        let sdt_data_len = sdt.data_len();

        let (header, entry_count) = match sdt_signature {
            sdt::RSDT_SIGNATURE => (AcpiHeader::Rsdt(sdt), sdt_data_len / mem::size_of::<u32>()),
            sdt::XSDT_SIGNATURE => (AcpiHeader::Xsdt(sdt), sdt_data_len / mem::size_of::<u64>()),

            _ => panic!("Invalid ACPI header signature: {:?}", sdt_signature),
        };

        Self {
            header,
            entry_count,
        }
    }

    /// Lookup ACPI table entry with the provided signature.
    fn lookup_entry(&self, signature: &str) -> Option<&'static Sdt> {
        let header_data_address = self.header.data_address() as *const u32;

        for i in 0..self.entry_count {
            // SAFTEY: Item address is valid as we are looping under the entry count and
            // the data address.
            let item_address = unsafe { *(header_data_address.add(i)) } as u64;
            let item = unsafe { Sdt::from_address(item_address) };

            if item.signature == signature.as_bytes() {
                return Some(item);
            }
        }

        None
    }
}

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct GenericAddressStructure {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

struct AmlHandler;

impl aml::Handler for AmlHandler {
    fn read_u8(&self, address: usize) -> u8 {
        log::trace!("AML: Reading byte from {:#x}", address);

        unsafe {
            (crate::PHYSICAL_MEMORY_OFFSET + address)
                .as_ptr::<u8>()
                .read_volatile()
        }
    }

    fn read_u16(&self, _address: usize) -> u16 {
        todo!()
    }

    fn read_u32(&self, _address: usize) -> u32 {
        todo!()
    }

    fn read_u64(&self, _address: usize) -> u64 {
        todo!()
    }

    fn write_u8(&mut self, _address: usize, _value: u8) {
        todo!()
    }

    fn write_u16(&mut self, _address: usize, _value: u16) {
        todo!()
    }

    fn write_u32(&mut self, _address: usize, _value: u32) {
        todo!()
    }

    fn write_u64(&mut self, _address: usize, _value: u64) {
        todo!()
    }

    fn read_io_u8(&self, _port: u16) -> u8 {
        todo!()
    }

    fn read_io_u16(&self, _port: u16) -> u16 {
        todo!()
    }

    fn read_io_u32(&self, _port: u16) -> u32 {
        todo!()
    }

    fn write_io_u8(&self, _port: u16, _value: u8) {
        todo!()
    }

    fn write_io_u16(&self, _port: u16, _value: u16) {
        todo!()
    }

    fn write_io_u32(&self, _port: u16, _value: u32) {
        todo!()
    }

    fn read_pci_u8(&self, _segment: u16, _bus: u8, _device: u8, _unction: u8, _offset: u16) -> u8 {
        todo!()
    }

    fn read_pci_u16(
        &self,
        _segment: u16,
        _bus: u8,
        _device: u8,
        _function: u8,
        _offset: u16,
    ) -> u16 {
        todo!()
    }

    fn read_pci_u32(
        &self,
        _segment: u16,
        _bus: u8,
        _device: u8,
        _function: u8,
        _offset: u16,
    ) -> u32 {
        todo!()
    }

    fn write_pci_u8(
        &self,
        _segment: u16,
        _bus: u8,
        _device: u8,
        _function: u8,
        _offset: u16,
        _value: u8,
    ) {
        todo!()
    }

    fn write_pci_u16(
        &self,
        _segment: u16,
        _bus: u8,
        _device: u8,
        _function: u8,
        _offset: u16,
        _value: u16,
    ) {
        todo!()
    }

    fn write_pci_u32(
        &self,
        _segment: u16,
        _bus: u8,
        _device: u8,
        _function: u8,
        _offset: u16,
        _value: u32,
    ) {
        todo!()
    }
}

/// Initialize the ACPI tables.
pub fn init(rsdp_address: PhysAddr, physical_memory_offset: VirtAddr) {
    let rsdp_address = physical_memory_offset + rsdp_address.as_u64();
    let acpi_table = AcpiTable::new(rsdp_address);

    macro init_table($sig:path => $ty:ty) {
        if let Some(table) = acpi_table.lookup_entry($sig) {
            <$ty>::new(table);
        }
    }

    if let Some(header) = acpi_table.lookup_entry(mcfg::SIGNATURE) {
        unsafe {
            let mcfg: &'static Mcfg = header.as_ptr();
            mcfg.init();
        }
    }

    if let Some(header) = acpi_table.lookup_entry(madt::SIGNATURE) {
        unsafe {
            // Not a valid MADT table without the local apic address and the flags.
            if header.data_len() < 8 {
                log::warn!(
                    "Assertion Failed: header.data_len() < 8 => {}",
                    header.data_len()
                );
            } else {
                let madt: &'static Madt = header.as_ptr();
                madt.init();
            }
        }
    }

    init_table!(fadt::SIGNATURE => Fadt);
    init_table!(hpet::SIGNATURE => Hpet);

    let aml_context = AmlContext::new(box AmlHandler, aml::DebugVerbosity::None);
}
