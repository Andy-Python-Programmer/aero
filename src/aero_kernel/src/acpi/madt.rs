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

use alloc::vec::Vec;
use spin::RwLock;

use crate::apic::IoApicHeader;

use super::sdt::Sdt;

pub(super) const SIGNATURE: &str = "APIC";

pub static IO_APICS: RwLock<Vec<&'static IoApicHeader>> = RwLock::new(Vec::new());
pub static ISOS: RwLock<Vec<&'static MadtIntSrcOverride>> = RwLock::new(Vec::new());

#[repr(C, packed)]
pub struct Madt {
    header: Sdt,
    local_apic_address: u32,
    flags: u32,
}

impl Madt {
    pub(super) fn init(&'static self) {
        // log::debug!("storing AP trampoline at 0x1000");

        // let page_index = unsafe { smp_prepare_trampoline() };

        for entry in self.iter() {
            match entry {
                MadtEntry::IoApic(e) => IO_APICS.write().push(e),
                MadtEntry::IntSrcOverride(e) => ISOS.write().push(e),
                _ => {}
            }
        }
    }

    fn iter(&self) -> MadtIterator {
        unsafe {
            MadtIterator {
                current: (self as *const _ as *const u8).add(mem::size_of::<Self>()),
                limit: (self as *const _ as *const u8).offset(self.header.length as isize),
            }
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct EntryHeader {
    entry_type: u8,
    length: u8,
}

#[repr(C, packed)]
struct MadtLocalApic {
    header: EntryHeader,
    processor_id: u8,
    apic_id: u8,
    flags: u32,
}

#[repr(C, packed)]
pub struct MadtIntSrcOverride {
    pub header: EntryHeader,
    pub bus: u8,
    pub irq: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}

enum MadtEntry {
    LocalApic(&'static MadtLocalApic),
    IoApic(&'static IoApicHeader),
    IntSrcOverride(&'static MadtIntSrcOverride),
}

struct MadtIterator {
    current: *const u8,
    limit: *const u8,
}

impl Iterator for MadtIterator {
    type Item = MadtEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.limit {
            unsafe {
                let entry_pointer = self.current;
                let header = *(self.current as *const EntryHeader);

                self.current = self.current.offset(header.length as isize);

                let item = match header.entry_type {
                    0 => MadtEntry::LocalApic(&*(entry_pointer as *const MadtLocalApic)),
                    1 => MadtEntry::IoApic(&*(entry_pointer as *const IoApicHeader)),
                    2 => MadtEntry::IntSrcOverride(&*(entry_pointer as *const MadtIntSrcOverride)),

                    0x10..=0x7f => continue,
                    0x80..=0xff => continue,

                    _ => {
                        log::warn!("unknown MADT entry with id: {}", header.entry_type);

                        return None;
                    }
                };

                return Some(item);
            }
        }

        None
    }
}
