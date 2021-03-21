use bootloader::BootInfo;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{OffsetPageTable, PageTable},
    VirtAddr,
};

/// Initialize paging.
pub fn init(boot_info: &BootInfo) -> OffsetPageTable {
    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let offset_table = unsafe { init_offset_page_table(physical_memory_offset) };

    offset_table
}

/// Initialize a new offset page table.
unsafe fn init_offset_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);

    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Get a mutable reference to the active level 4 page table.
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let physical = level_4_table_frame.start_address();
    let virtual_address = physical_memory_offset + physical.as_u64();
    let page_table_ptr: *mut PageTable = virtual_address.as_mut_ptr();

    &mut *page_table_ptr
}
