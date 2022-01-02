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

use spin::Once;

use super::sdt::Sdt;

pub(super) const SIGNATURE: &str = "MCFG";

static MCFG: Once<&'static Mcfg> = Once::new();

#[repr(C, packed)]
pub struct DeviceConfig {
    pub base_address: u64,
    pub pci_seg_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
    pub reserved: u32,
}

/// The ACPI MCFG table describes the location of the PCI Express configuration space.
#[repr(C, packed)]
pub struct Mcfg {
    header: Sdt,
    reserved: u64,
}

impl Mcfg {
    pub(super) unsafe fn init(&'static self) {
        MCFG.call_once(move || self);
    }

    pub fn entry_count(&self) -> usize {
        (self.header.length as usize - mem::size_of::<Self>()) / mem::size_of::<DeviceConfig>()
    }
}

/// Returns true if the ACPI table contains the MCFG entry.
///
/// ## Notes
/// Returns false if called before the ACPI tables were initialized.
pub fn is_avaliable() -> bool {
    MCFG.get().is_some()
}

/// Return a immutable reference to the MCFG table.
pub fn get_mcfg_table() -> &'static Mcfg {
    MCFG.get()
        .expect("Attempted to get the MCFG table before it was initialized")
}
