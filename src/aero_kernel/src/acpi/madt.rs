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

use core::alloc::Layout;
use core::mem;

use core::sync::atomic::Ordering;

use alloc::alloc::alloc_zeroed;

use alloc::vec::Vec;
use spin::RwLock;

use crate::apic;
use crate::apic::ApicType;
use crate::arch::controlregs;
use crate::arch::interrupts;

use crate::apic::IoApicHeader;
use crate::apic::CPU_COUNT;
use crate::mem::paging;
use crate::utils::io;

use super::sdt::Sdt;

pub(super) const SIGNATURE: &str = "APIC";

extern "C" {
    fn smp_prepare_trampoline() -> u16;
    fn smp_prepare_launch(page_table: u64, stack_top: u64, ap_id: u64, mode: u32);
    fn smp_check_ap_flag() -> bool;
}

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
        log::debug!("Storing AP trampoline at 0x1000");

        let page_index = unsafe { smp_prepare_trampoline() };

        for entry in self.iter() {
            match entry {
                MadtEntry::LocalApic(local_apic) => {
                    // Make sure that we can actually start the application processor.
                    if (!((local_apic.flags & 1) ^ ((local_apic.flags >> 1) & 1))) == 1 {
                        log::warn!("Unable to start AP{}", local_apic.apic_id);
                        continue;
                    }

                    // Increase the CPU count.
                    CPU_COUNT.fetch_add(1, Ordering::SeqCst);

                    // Do not restart the BSP.
                    if local_apic.apic_id == apic::get_bsp_id() as u8 {
                        continue;
                    }

                    let page_table = controlregs::read_cr3_raw();
                    let stack_top = unsafe {
                        let layout = Layout::from_size_align_unchecked(4096 * 16, 4096);
                        let raw = alloc_zeroed(layout);

                        raw.add(layout.size())
                    };

                    let mode = if paging::level_5_paging_enabled() {
                        1 << 1
                    } else {
                        0 << 1
                    };

                    unsafe {
                        smp_prepare_launch(
                            page_table,
                            stack_top as u64,
                            local_apic.apic_id as u64,
                            mode,
                        );
                    }

                    apic::mark_ap_ready(false);

                    let mut bsp = apic::get_local_apic();

                    // Send the init IPI.
                    unsafe {
                        if bsp.apic_type() == ApicType::X2apic {
                            bsp.set_icr_x2apic(((local_apic.apic_id as u64) << 32) | 0x4500);
                        } else {
                            bsp.set_icr_xapic((local_apic.apic_id as u32) << 24, 0x4500);
                        }
                    }

                    io::delay(5000);

                    // Send the startup IPI.
                    unsafe {
                        if bsp.apic_type() == ApicType::X2apic {
                            bsp.set_icr_x2apic(
                                ((local_apic.apic_id as u64) << 32) | (page_index | 0x4600) as u64,
                            );
                        } else {
                            bsp.set_icr_xapic(
                                ((local_apic.apic_id as u64) << 24) as u32,
                                (page_index | 0x4600) as u32,
                            )
                        }
                    }

                    unsafe {
                        // Wait for the AP to be ready.
                        for _ in 0..100 {
                            if smp_check_ap_flag() {
                                break;
                            }

                            io::delay(10000)
                        }
                    }

                    // Wait for the trampoline to be ready.
                    while !apic::ap_ready() {
                        interrupts::pause();
                    }
                }

                MadtEntry::IoApic(e) => IO_APICS.write().push(e),
                MadtEntry::IntSrcOverride(e) => ISOS.write().push(e),
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
                        log::warn!("Unknown MADT entry with id: {}", header.entry_type);

                        return None;
                    }
                };

                return Some(item);
            }
        }

        None
    }
}
