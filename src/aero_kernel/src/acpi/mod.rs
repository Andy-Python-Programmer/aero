// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

//! The ACPI (Advanced Configuration and Power Interface) tables help to gather the
//! CPU, interrupt, and timer information.
//!
//! **Notes**: <https://wiki.osdev.org/ACPI>

use spin::Once;

use crate::mem::paging::VirtAddr;
use crate::utils::sync::{Mutex, MutexGuard};

use self::hpet::Hpet;
use self::madt::Madt;
use self::mcfg::Mcfg;
use self::sdt::Sdt;

pub mod aml;
pub mod fadt;
pub mod hpet;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod sdt;

enum AcpiHeader {
    Rsdt(&'static rsdp::Rsdt<u32>),
    Xsdt(&'static rsdp::Rsdt<u64>),
}

pub struct AcpiTable {
    header: AcpiHeader,
}

impl AcpiTable {
    fn new(rsdp_address: VirtAddr) -> Self {
        match rsdp::find_rsdt_address(rsdp_address) {
            rsdp::RsdtAddress::Xsdt(xsdt_addr) => {
                let xsdt = rsdp::Rsdt::<u64>::new(xsdt_addr);
                let header = AcpiHeader::Xsdt(xsdt);

                log::debug!("found XSDT at {:#x}", xsdt_addr);

                Self { header }
            }

            rsdp::RsdtAddress::Rsdt(rsdt_addr) => {
                let rsdt = rsdp::Rsdt::<u32>::new(rsdt_addr);
                let header = AcpiHeader::Rsdt(rsdt);

                log::debug!("found RSDT at {:#x}", rsdt_addr);

                Self { header }
            }
        }
    }

    /// Lookup ACPI table entry with the provided signature.
    pub fn lookup_entry(&self, signature: &str, index: usize) -> Option<&'static Sdt> {
        match self.header {
            AcpiHeader::Rsdt(rsdt) => rsdt.lookup_entry(signature, index),
            AcpiHeader::Xsdt(xsdt) => xsdt.lookup_entry(signature, index),
        }
    }

    pub fn revision(&self) -> u8 {
        match self.header {
            AcpiHeader::Rsdt(rsdt) => rsdt.header.revision,
            AcpiHeader::Xsdt(xsdt) => xsdt.header.revision,
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct GenericAddressStructure {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

static ACPI_TABLE: Once<Mutex<AcpiTable>> = Once::new();

pub fn get_acpi_table() -> MutexGuard<'static, AcpiTable> {
    ACPI_TABLE.get().unwrap().lock()
}

/// Initialize the ACPI tables.
pub fn init(rsdp_address: VirtAddr) {
    let acpi_table = AcpiTable::new(rsdp_address);

    ACPI_TABLE.call_once(|| Mutex::new(acpi_table));

    let acpi_table = get_acpi_table();

    macro init_table($sig:path => $ty:ty) {
        if let Some(table) = acpi_table.lookup_entry($sig, 0) {
            <$ty>::new(table);
        }
    }

    if let Some(header) = acpi_table.lookup_entry(mcfg::SIGNATURE, 0) {
        unsafe {
            let mcfg: &'static Mcfg = header.as_ref();
            mcfg.init();
        }
    }

    if let Some(header) = acpi_table.lookup_entry(madt::SIGNATURE, 0) {
        unsafe {
            // Not a valid MADT table without the local apic address and the flags.
            if header.data_len() < 8 {
                log::warn!(
                    "assertion failed: header.data_len() < 8 => {}",
                    header.data_len()
                );
            } else {
                let madt: &'static Madt = header.as_ref();
                madt.init();
            }
        }
    }

    init_table!(hpet::SIGNATURE => Hpet);
}
