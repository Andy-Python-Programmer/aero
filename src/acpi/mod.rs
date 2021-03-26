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

use self::{rsdp::RSDP, sdt::SDT};

pub mod mcfg;
pub mod rsdp;
pub mod sdt;

unsafe fn look_up_tables(
    sdt: &'static SDT,
    is_legacy: bool,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    offset_table: &mut OffsetPageTable,
) {
    let entries;

    if is_legacy {
        entries = sdt.data_len() / mem::size_of::<u32>()
    } else {
        entries = sdt.data_len() / mem::size_of::<u64>()
    }

    for i in 0..entries {
        let item_address = *((sdt.data_address() as *const u32).add(i));
        let item = SDT::from_address(item_address as u64, frame_allocator, offset_table);

        crate::dbg!(item.get_signature());
    }
}

/// Initialize ACPI tables.
pub fn init(
    offset_table: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    unsafe {
        let start_frame: PhysFrame<Size4KiB> =
            PhysFrame::containing_address(PhysAddr::new(LOOKUP_START_ADDRESS as u64));
        let end_frame = PhysFrame::containing_address(PhysAddr::new(LOOKUP_END_ADDRESS as u64));

        // Map all of the ACPI table space.
        for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
            let page: Page<Size4KiB> =
                Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));

            if offset_table.translate_page(page).is_err() {
                let _ = offset_table
                    .identity_map(
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                        frame_allocator,
                    )
                    .unwrap();
            }
        }

        let rsdp = RSDP::lookup(LOOKUP_START_ADDRESS, LOOKUP_END_ADDRESS);

        if let Some(rsdp) = rsdp {
            let sdt_address = rsdp.get_sdt_address() as u64;
            let sdt = SDT::from_address(sdt_address, frame_allocator, offset_table);

            let is_leagcy;

            if sdt.get_signature() == "XSDT" {
                is_leagcy = false;
            } else if sdt.get_signature() == "RSDT" {
                is_leagcy = true;
            } else {
                panic!("Invalid RSDP signature.")
            }

            look_up_tables(sdt, is_leagcy, frame_allocator, offset_table);
        } else {
            panic!("Unable to find the RSDP")
        }
    }
}
