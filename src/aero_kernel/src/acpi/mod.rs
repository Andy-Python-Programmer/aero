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

//! The ACPI (Advanced Configuration and Power Interface) tables help to gather the
//! CPU, interrupt, and timer informations.
//!
//! **Notes**: <https://wiki.osdev.org/ACPI>

use aml::AmlContext;
use spin::Once;

use crate::mem::paging::{PhysAddr, VirtAddr};

use crate::utils::sync::{Mutex, MutexGuard};

use self::{hpet::Hpet, madt::Madt, mcfg::Mcfg, sdt::Sdt};

pub mod fadt;
pub mod hpet;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod sdt;

static AML_CONTEXT: Once<Mutex<AmlContext>> = Once::new();

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

                log::debug!("Found XSDT at {:#x}", xsdt_addr);

                Self { header }
            }

            rsdp::RsdtAddress::Rsdt(rsdt_addr) => {
                let rsdt = rsdp::Rsdt::<u32>::new(rsdt_addr);
                let header = AcpiHeader::Rsdt(rsdt);

                log::debug!("Found RSDT at {:#x}", rsdt_addr);

                Self { header }
            }
        }
    }

    /// Lookup ACPI table entry with the provided signature.
    fn lookup_entry(&self, signature: &str) -> Option<&'static Sdt> {
        match self.header {
            AcpiHeader::Rsdt(rsdt) => rsdt.lookup_entry(signature),
            AcpiHeader::Xsdt(xsdt) => xsdt.lookup_entry(signature),
        }
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
pub fn init(rsdp_address: PhysAddr) -> Result<(), aml::AmlError> {
    let rsdp_address = unsafe { crate::PHYSICAL_MEMORY_OFFSET + rsdp_address.as_u64() };
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
                    "assertion failed: header.data_len() < 8 => {}",
                    header.data_len()
                );
            } else {
                let madt: &'static Madt = header.as_ptr();
                madt.init();
            }
        }
    }

    init_table!(hpet::SIGNATURE => Hpet);

    let aml_context = AmlContext::new(box AmlHandler, aml::DebugVerbosity::None);

    if let Some(fadt) = acpi_table.lookup_entry(fadt::SIGNATURE) {
        let fadt: &'static fadt::Fadt = unsafe { fadt.as_ptr() };

        // The DSDT table is put inside the FADT table, instead of listing it in another ACPI table. So
        // we need to extract the DSDT table from the FADT table.
        let _dsdt_stream = unsafe {
            let addr = crate::PHYSICAL_MEMORY_OFFSET + fadt.dsdt as u64;
            let sdt = Sdt::from_address(addr);

            core::slice::from_raw_parts(sdt.data_address() as *mut u8, sdt.data_len())
        };

        // aml_context.parse_table(dsdt_stream)?;
    }

    // let pci_router =
    //     PciRoutingTable::from_prt_path(&AmlName::from_str("\\_SB.PCI0._PRT")?, &mut aml_context)?;

    // drivers::pci::init_pci_router(pci_router);

    AML_CONTEXT.call_once(move || Mutex::new(aml_context));
    Ok(())
}

/// Returns an immutable reference to the AML context.
pub fn get_aml_context() -> MutexGuard<'static, AmlContext> {
    AML_CONTEXT
        .get()
        .expect("attempted to get AML context before the ACPI initialization")
        .lock()
}
