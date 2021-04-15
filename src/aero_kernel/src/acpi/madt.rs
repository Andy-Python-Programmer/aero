use core::{intrinsics, mem};

use x86_64::{structures::paging::*, PhysAddr, VirtAddr};

use super::sdt::SDT;

pub const SIGNATURE: &str = "APIC";
pub const TRAMPOLINE: u64 = 0x8000;

static TRAMPOLINE_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline"));
static mut MADT: Option<MADT> = None;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct MADT {
    pub sdt: &'static SDT,
    pub local_apic_address: u32,
    pub flags: u32,
}

impl MADT {
    pub fn new(
        sdt: Option<&'static SDT>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        offset_table: &mut OffsetPageTable,
    ) {
        if let Some(sdt) = sdt {
            // Not a valid MADT table without the local apic address and the flags.
            if sdt.data_len() < 8 {
                return;
            }

            let local_apic_address = unsafe { *(sdt.data_address() as *const u32) };
            let flags = unsafe { *(sdt.data_address() as *const u32).offset(1) };

            let madt = Self {
                sdt,
                local_apic_address,
                flags,
            };

            unsafe { MADT = Some(madt) };

            log::info!("Enabling multicore");

            unsafe {
                let trampoline_frame = PhysFrame::containing_address(PhysAddr::new(TRAMPOLINE));
                let trampoline_page: Page<Size4KiB> =
                    Page::containing_address(VirtAddr::new(TRAMPOLINE));

                offset_table
                    .map_to(
                        trampoline_page,
                        trampoline_frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .unwrap()
                    .flush();

                // Atomic store the AP trampoline code to a fixed address in low conventional memory.
                for i in 0..TRAMPOLINE_BIN.len() {
                    intrinsics::atomic_store((TRAMPOLINE as *mut u8).add(i), TRAMPOLINE_BIN[i]);
                }
            }
        }
    }

    pub fn iter(&self) -> MADTIterator {
        unsafe {
            MADTIterator {
                ptr: (self as *const Self as *const u8).add(mem::size_of::<Self>()),
                i: self.sdt.length as usize - mem::size_of::<Self>(),
            }
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct EntryHeader {
    pub entry_type: u8,
    pub length: u8,
}

#[repr(C, packed)]
pub struct MadtLocalApic {
    pub header: EntryHeader,
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

#[repr(C, packed)]
pub struct MadtIoApic {
    pub header: EntryHeader,
    pub io_apic_id: u8,
    reserved: u8,
    pub io_apic_address: u32,
    pub global_system_interrupt_base: u32,
}

#[repr(C, packed)]
pub struct MadtIntSrcOverride {
    pub header: EntryHeader,
    pub bus: u8,
    pub irq: u8,
    pub global_system_interrupt: u32,
    pub flags: u16,
}

pub enum MADTEntry {
    LocalApic(&'static MadtLocalApic),
    IOApic(&'static MadtIoApic),
    IntSrcOverride(&'static MadtIntSrcOverride),

    Unknown(u8),
}

pub struct MADTIterator {
    ptr: *const u8,
    i: usize,
}

impl Iterator for MADTIterator {
    type Item = MADTEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while self.i > 0 {
            unsafe {
                let entry_pointer = self.ptr;
                let header = *(self.ptr as *const EntryHeader);

                self.ptr = self.ptr.offset(header.length.into());
                self.i -= header.length as usize;

                let item = match header.entry_type {
                    0 => MADTEntry::LocalApic(&*(entry_pointer as *const MadtLocalApic)),
                    1 => MADTEntry::IOApic(&*(entry_pointer as *const MadtIoApic)),
                    2 => MADTEntry::IntSrcOverride(&*(entry_pointer as *const MadtIntSrcOverride)),

                    0x10..=0x7f => continue,
                    0x80..=0xff => continue,

                    _ => MADTEntry::Unknown(header.entry_type),
                };

                return Some(item);
            }
        }

        None
    }
}
