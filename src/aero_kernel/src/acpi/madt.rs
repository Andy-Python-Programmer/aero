use core::mem;

use spin::Once;
use x86_64::{structures::paging::*, PhysAddr, VirtAddr};

use super::sdt::Sdt;
use crate::apic;

pub const SIGNATURE: &str = "APIC";
pub const TRAMPOLINE: u64 = 0x8000;

static MADT: Once<&'static Madt> = Once::new();
static TRAMPOLINE_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline"));

#[derive(Clone, Copy, Debug)]
pub struct Madt {
    pub header: Sdt,
    pub local_apic_address: u32,
    pub flags: u32,
}

impl Madt {
    pub(super) fn init(
        &'static self,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        offset_table: &mut OffsetPageTable,
    ) {
        MADT.call_once(move || self);

        log::info!("Enabling multicore");

        let trampoline_frame = PhysFrame::containing_address(PhysAddr::new(TRAMPOLINE));
        let trampoline_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(TRAMPOLINE));

        unsafe {
            offset_table
                .map_to(
                    trampoline_page,
                    trampoline_frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .flush();
        }

        // Atomic store the AP trampoline code to a fixed address in low conventional memory.
        // for i in 0..TRAMPOLINE_BIN.len() {
        //     intrinsics::atomic_store((TRAMPOLINE as *mut u8).add(i), TRAMPOLINE_BIN[i]);
        // }

        for stuff in self.iter() {
            match stuff {
                MadtEntry::LocalApic(local_apic) => {
                    if local_apic.apic_id == apic::get_bsp_id() as u8 {
                        // We do not want to start the BSP that is already running
                        // this code :D
                        continue;
                    }
                }
                MadtEntry::IoApic(_) => {}
                MadtEntry::IntSrcOverride(_) => {}
            }
        }
    }

    pub fn iter(&self) -> MadtIterator {
        unsafe {
            MadtIterator {
                current: (self as *const Self as *const u8).add(mem::size_of::<Self>()),
                limit: (self as *const _ as *const u8).offset(self.header.length as isize),
            }
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct MadtEntryHeader {
    pub entry_type: u8,
    pub length: u8,
}

#[repr(C, packed)]
pub struct MadtLocalApic {
    pub header: MadtEntryHeader,
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

#[repr(C, packed)]
pub struct MadtIoApic {
    pub header: MadtEntryHeader,
    pub io_apic_id: u8,
    reserved: u8,
    pub io_apic_address: u32,
    pub global_system_interrupt_base: u32,
}

#[repr(C, packed)]
pub struct MadtIntSrcOverride {
    pub header: MadtEntryHeader,
    pub bus: u8,
    pub irq: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}

pub enum MadtEntry {
    LocalApic(&'static MadtLocalApic),
    IoApic(&'static MadtIoApic),
    IntSrcOverride(&'static MadtIntSrcOverride),
}

pub struct MadtIterator {
    current: *const u8,
    limit: *const u8,
}

impl Iterator for MadtIterator {
    type Item = MadtEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.limit {
            unsafe {
                let entry_pointer = self.current;
                let header = *(self.current as *const MadtEntryHeader);

                self.current = self.current.offset(header.length as isize);

                let item = match header.entry_type {
                    0 => MadtEntry::LocalApic(&*(entry_pointer as *const MadtLocalApic)),
                    1 => MadtEntry::IoApic(&*(entry_pointer as *const MadtIoApic)),
                    2 => MadtEntry::IntSrcOverride(&*(entry_pointer as *const MadtIntSrcOverride)),

                    0x10..=0x7f => continue,
                    0x80..=0xff => continue,

                    _ => {
                        log::warn!("Unknown MADT entry with id {}", header.entry_type);

                        return None;
                    }
                };

                return Some(item);
            }
        }

        None
    }
}
