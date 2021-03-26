//! The ACPI (Advanced Configuration and Power Interface) tables help to gather the
//! CPU, interrupt, and timer informations.
//!
//! **Notes**: <https://wiki.osdev.org/ACPI>

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
            let rsdp_frame: PhysFrame<Size4KiB> =
                PhysFrame::containing_address(PhysAddr::new(rsdp.get_sdt_address() as u64));

            let sdt_address = rsdp.get_sdt_address() as u64;

            let _ = offset_table
                .identity_map(
                    rsdp_frame,
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                    frame_allocator,
                )
                .unwrap();

            let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(sdt_address));

            if offset_table.translate_page(page).is_err() {
                let sdt_frame: PhysFrame<Size4KiB> =
                    PhysFrame::containing_address(PhysAddr::new(sdt_address));

                let _ = offset_table
                    .identity_map(
                        sdt_frame,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                        frame_allocator,
                    )
                    .unwrap();
            }

            let sdt = &*(sdt_address as *const SDT);
        } else {
            panic!("Unable to find the RSDP")
        }
    }
}
