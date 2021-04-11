use uefi::{
    prelude::*,
    table::boot::{AllocateType, MemoryType},
};

use x86_64::{
    registers::{
        control::{Cr0, Cr0Flags, Cr3, Cr3Flags},
        model_specific::{Efer, EferFlags},
    },
    structures::paging::*,
    PhysAddr, VirtAddr,
};

pub struct BootFrameAllocator<'a>(&'a BootServices);

impl<'a> BootFrameAllocator<'a> {
    pub fn new(boot_services: &'a BootServices) -> Self {
        Self(boot_services)
    }
}

unsafe impl<'a> FrameAllocator<Size4KiB> for BootFrameAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let address = self
            .0
            .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
            .expect_success("Failed to allocate physical frame");

        let address = PhysAddr::new(address);
        let frame = PhysFrame::containing_address(address);

        Some(frame)
    }
}

pub struct PageTables {
    pub boot_page_table: OffsetPageTable<'static>,
    pub kernel_page_table: OffsetPageTable<'static>,
    pub kernel_level_4_frame: PhysFrame,
}

pub fn init(frame_allocator: &mut BootFrameAllocator) -> PageTables {
    let physical_offset = VirtAddr::new(0x00);

    let old_table = {
        let frame = Cr3::read().0;
        let ptr: *const PageTable = (physical_offset + frame.start_address().as_u64()).as_ptr();

        unsafe { &*ptr }
    };

    let new_frame = frame_allocator.allocate_frame().unwrap();

    let new_table: &mut PageTable = {
        let ptr: *mut PageTable =
            (physical_offset + new_frame.start_address().as_u64()).as_mut_ptr();

        unsafe {
            ptr.write(PageTable::new());

            &mut *ptr
        }
    };

    // Copy the first entry (we don't need to access more than 512 GiB; also, some UEFI
    // implementations seem to create an level 4 table entry 0 in all slots)
    new_table[0] = old_table[0].clone();

    let boot_page_table = unsafe {
        Cr3::write(new_frame, Cr3Flags::empty());
        OffsetPageTable::new(&mut *new_table, physical_offset)
    };

    let (kernel_page_table, kernel_level_4_frame) = {
        let frame: PhysFrame = frame_allocator.allocate_frame().expect("no unused frames");
        log::info!("Created a new page table for the kernel at: {:#?}", &frame);

        let addr = physical_offset + frame.start_address().as_u64();

        // Initialize a new page table.
        let ptr = addr.as_mut_ptr();
        unsafe { *ptr = PageTable::new() };

        let level_4_table = unsafe { &mut *ptr };
        (
            unsafe { OffsetPageTable::new(level_4_table, physical_offset) },
            frame,
        )
    };

    PageTables {
        boot_page_table,
        kernel_page_table,
        kernel_level_4_frame,
    }
}

pub fn enable_no_execute() {
    unsafe { Efer::update(|efer| *efer |= EferFlags::NO_EXECUTE_ENABLE) }
}

pub fn enable_protection() {
    unsafe { Cr0::update(|cr0| *cr0 |= Cr0Flags::WRITE_PROTECT) };
}
