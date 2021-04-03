use core::{mem, ptr};

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags,
        PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use super::sdt::SDT;

pub static mut MADT: Option<MADT> = None;

pub const SIGNATURE: &str = "APIC";
pub const TRAMPOLINE: u64 = 0x8000;

#[derive(Clone, Copy, Debug)]
pub struct MADT {
    pub sdt: &'static SDT,
    pub local_address: u32,
    pub flags: u32,
}

impl MADT {
    pub fn new(
        sdt: Option<&'static SDT>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        offset_table: &mut OffsetPageTable,
    ) {
        if let Some(sdt) = sdt {
            log::info!("Enabling multicore");

            unsafe {
                let madt = ptr::read((sdt as *const SDT) as *const Self);

                MADT = Some(madt);

                let trampoline_frame = PhysFrame::containing_address(PhysAddr::new(TRAMPOLINE));
                let trampoline_page: Page<Size4KiB> =
                    Page::containing_address(VirtAddr::new(TRAMPOLINE));

                match offset_table.map_to(
                    trampoline_page,
                    trampoline_frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                ) {
                    Ok(toilet) => toilet.flush(),
                    Err(err) => match err {
                        MapToError::PageAlreadyMapped(_) => (),
                        _ => panic!("{:?}", err),
                    },
                }
            }
        }
    }

    #[allow(unused)]
    pub fn iter(&self) -> MADTIterator {
        MADTIterator {
            sdt: self.sdt,
            i: 8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct MadtLocalApic {
    pub processor_id: u8,
    pub local_apic_id: u8,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct MadtIoApic {
    pub id: u8,
    reserved: u8,
    pub address: u32,
    pub gsi_base: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct MadtIntSrcOverride {
    pub bus_source: u8,
    pub irq_source: u8,
    pub gsi_base: u32,
    pub flags: u16,
}

#[derive(Debug)]
pub enum MADTEntry {
    LocalApic(&'static MadtLocalApic),
    InvalidLocalApic(usize),

    IOApic(&'static MadtIoApic),
    InvalidIoApic(usize),

    IntSrcOverride(&'static MadtIntSrcOverride),
    InvalidIntSrcOverride(usize),

    Unknown(u8),
}

pub struct MADTIterator {
    sdt: &'static SDT,
    i: usize,
}

impl Iterator for MADTIterator {
    type Item = MADTEntry;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i + 1 < self.sdt.data_len() {
            let entry_type = unsafe { *(self.sdt.data_address() as *const u8).add(self.i) };
            let entry_len =
                unsafe { *(self.sdt.data_address() as *const u8).add(self.i + 1) } as usize;

            if self.i + entry_len <= self.sdt.data_len() {
                let item = match entry_type {
                    0 => {
                        if entry_len == mem::size_of::<MadtLocalApic>() + 2 {
                            MADTEntry::LocalApic(unsafe {
                                &*((self.sdt.data_address() + self.i + 2) as *const MadtLocalApic)
                            })
                        } else {
                            MADTEntry::InvalidLocalApic(entry_len)
                        }
                    }
                    1 => {
                        if entry_len == mem::size_of::<MadtIoApic>() + 2 {
                            MADTEntry::IOApic(unsafe {
                                &*((self.sdt.data_address() + self.i + 2) as *const MadtIoApic)
                            })
                        } else {
                            MADTEntry::InvalidIoApic(entry_len)
                        }
                    }
                    2 => {
                        if entry_len == mem::size_of::<MadtIntSrcOverride>() + 2 {
                            MADTEntry::IntSrcOverride(unsafe {
                                &*((self.sdt.data_address() + self.i + 2)
                                    as *const MadtIntSrcOverride)
                            })
                        } else {
                            MADTEntry::InvalidIntSrcOverride(entry_len)
                        }
                    }
                    _ => MADTEntry::Unknown(entry_type),
                };

                self.i += entry_len;

                Some(item)
            } else {
                None
            }
        } else {
            None
        }
    }
}
