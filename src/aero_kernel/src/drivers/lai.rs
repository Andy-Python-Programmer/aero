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

use alloc::sync::Arc;

use crate::acpi::aml;
use crate::acpi::fadt;
use crate::acpi::get_acpi_table;

use crate::mem::paging::PhysAddr;

use crate::arch::io;
use crate::userland::scheduler;

use super::pci::PciHeader;

struct LaiHost;

impl lai::Host for LaiHost {
    fn scan(&self, signature: &str, index: usize) -> *const u8 {
        assert!(index == 0);

        if signature == "DSDT" {
            // The DSDT table is put inside the FADT table, instead of listing it in
            // another ACPI table. So, we need to extract the DSDT table from the FADT
            // table.
            get_acpi_table().lookup_entry(fadt::SIGNATURE).map(|fadt| {
                let fadt: &'static fadt::Fadt = unsafe { fadt.as_ref() };
                let addr = PhysAddr::new(fadt.dsdt as u64).as_hhdm_virt();
                addr.as_ptr::<u8>()
            })
        } else {
            get_acpi_table()
                .lookup_entry(signature)
                .map(|table| table as *const _ as *const u8)
        }
        .unwrap_or(core::ptr::null())
    }

    fn sleep(&self, ms: u64) {
        scheduler::get_scheduler()
            .inner
            .sleep(Some(ms as usize / 1000))
            .expect("lai: unexpected signal during sleep")
    }

    // Port I/O functions:
    fn outb(&self, port: u16, value: u8) {
        unsafe { io::outb(port, value) }
    }

    fn outw(&self, port: u16, value: u16) {
        unsafe { io::outw(port, value) }
    }

    fn outd(&self, port: u16, value: u32) {
        unsafe { io::outl(port, value) }
    }

    fn inb(&self, port: u16) -> u8 {
        unsafe { io::inb(port) }
    }

    fn inw(&self, port: u16) -> u16 {
        unsafe { io::inw(port) }
    }

    fn ind(&self, port: u16) -> u32 {
        unsafe { io::inl(port) }
    }

    // PCI read functions:
    //
    // todo: do not ignore the segment once we use MCFG.
    fn pci_readb(&self, _seg: u16, bus: u8, slot: u8, fun: u8, offset: u16) -> u8 {
        let header = PciHeader::new(bus, slot, fun);
        unsafe { header.read::<u8>(offset as u32) as u8 }
    }

    fn pci_readw(&self, _seg: u16, bus: u8, slot: u8, fun: u8, offset: u16) -> u16 {
        let header = PciHeader::new(bus, slot, fun);
        unsafe { header.read::<u16>(offset as u32) as u16 }
    }

    fn pci_readd(&self, _seg: u16, bus: u8, slot: u8, fun: u8, offset: u16) -> u32 {
        let header = PciHeader::new(bus, slot, fun);
        unsafe { header.read::<u32>(offset as u32) }
    }

    // Memory functions:
    fn map(&self, address: usize, _count: usize) -> *mut u8 {
        PhysAddr::new(address as u64)
            .as_hhdm_virt()
            .as_mut_ptr::<u8>()
    }
}

struct LaiSubsystem;

impl aml::AmlSubsystem for LaiSubsystem {
    fn enter_state(&self, state: aml::SleepState) {
        lai::enter_sleep(state as u8)
    }

    fn enable_acpi(&self, mode: u32) {
        lai::enable_acpi(mode);
    }
}

pub fn init_lai() {
    let lai_host = Arc::new(LaiHost);
    lai::init(lai_host);

    lai::set_acpi_revision(get_acpi_table().revision() as _);
    lai::create_namespace();

    let subsystem = Arc::new(LaiSubsystem);
    aml::init(subsystem);
}

crate::module_init!(init_lai, ModuleType::Other);
