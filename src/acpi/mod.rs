//! The ACPI (Advanced Configuration and Power Interface) tables help to gather the
//! CPU, interrupt, and timer informations.
//!
//! **Notes**: <https://wiki.osdev.org/ACPI>

use core::mem;

use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

const LOOKUP_START_ADDRESS: usize = 0xE0000;
const LOOKUP_END_ADDRESS: usize = 0xFFFFF;

use crate::arch::memory::paging::GlobalAllocator;

use self::{fadt::FADT, hpet::HPET, madt::MADT, rsdp::RSDP, sdt::SDT};

pub mod fadt;
pub mod hpet;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod sdt;

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct GenericAddressStructure {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

impl GenericAddressStructure {
    pub unsafe fn init(
        &self,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        offset_table: &mut OffsetPageTable,
    ) {
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(self.address));
        let frame = PhysFrame::containing_address(PhysAddr::new(self.address));

        offset_table
            .map_to(
                page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
                frame_allocator,
            )
            .unwrap()
            .flush();
    }
}

unsafe fn look_up_table(
    signature: &str,
    sdt: &'static SDT,
    is_legacy: bool,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    offset_table: &mut OffsetPageTable,
) -> Option<&'static SDT> {
    let entries;

    if is_legacy {
        entries = sdt.data_len() / mem::size_of::<u32>()
    } else {
        entries = sdt.data_len() / mem::size_of::<u64>()
    }

    for i in 0..entries {
        let item_address = *((sdt.data_address() as *const u32).add(i));
        let item = SDT::from_address(item_address as u64, frame_allocator, offset_table);

        if item.get_signature() == signature {
            return Some(item);
        }
    }

    None
}

/// Initialize ACPI tables.
pub fn init(offset_table: &mut OffsetPageTable, frame_allocator: &mut GlobalAllocator) {
    unsafe {
        let start_frame: PhysFrame<Size4KiB> =
            PhysFrame::containing_address(PhysAddr::new(LOOKUP_START_ADDRESS as u64));
        let end_frame = PhysFrame::containing_address(PhysAddr::new(LOOKUP_END_ADDRESS as u64));

        // Map all of the ACPI table space.
        for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
            let page: Page<Size4KiB> =
                Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));

            if offset_table.translate_page(page).is_err() {
                offset_table
                    .identity_map(
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                        frame_allocator,
                    )
                    .unwrap()
                    .flush();
            }
        }

        let rsdp = RSDP::lookup(LOOKUP_START_ADDRESS, LOOKUP_END_ADDRESS);

        if let Some(rsdp) = rsdp {
            let sdt_address = rsdp.get_sdt_address() as u64;
            let sdt = SDT::from_address(sdt_address, frame_allocator, offset_table);

            let is_legacy;

            if sdt.get_signature() == "XSDT" {
                is_legacy = false;
            } else if sdt.get_signature() == "RSDT" {
                is_legacy = true;
            } else {
                panic!("Invalid RSDP signature.")
            }

            FADT::new(look_up_table(
                fadt::SIGNATURE,
                sdt,
                is_legacy,
                frame_allocator,
                offset_table,
            ));

            HPET::new(
                look_up_table(
                    hpet::SIGNATURE,
                    sdt,
                    is_legacy,
                    frame_allocator,
                    offset_table,
                ),
                frame_allocator,
                offset_table,
            );

            MADT::new(
                look_up_table(
                    madt::SIGNATURE,
                    sdt,
                    is_legacy,
                    frame_allocator,
                    offset_table,
                ),
                frame_allocator,
                offset_table,
            );
        } else {
            panic!("Unable to find the RSDP")
        }
    }
}
