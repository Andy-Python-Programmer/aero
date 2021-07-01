/*
 * Copyright (C) 2021 The Aero Project Developers.
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

pub mod frame;

use stivale::v2::{StivaleMemoryMapEntryType, StivaleMemoryMapTag};

use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{mapper::MapToError, *};
use x86_64::VirtAddr;

pub use frame::LockedFrameAllocator;

use crate::PHYSICAL_MEMORY_OFFSET;

pub static mut FRAME_ALLOCATOR: LockedFrameAllocator = LockedFrameAllocator::new_uninit();

pub struct UnmapGuard {
    pub page: Page<Size4KiB>,
}

impl UnmapGuard {
    #[inline]
    fn new(page: Page<Size4KiB>) -> Self {
        Self { page }
    }
}

/// Initialize paging.
pub fn init(
    memory_regions: &'static StivaleMemoryMapTag,
) -> Result<OffsetPageTable<'static>, MapToError<Size4KiB>> {
    let active_level_4 = unsafe { active_level_4_table() };
    let offset_table = unsafe { OffsetPageTable::new(active_level_4, PHYSICAL_MEMORY_OFFSET) };

    unsafe {
        FRAME_ALLOCATOR.init(memory_regions);
    }

    /*
     * Create a new page table for the kernel rather then using the one provided
     * by the bootloader. This allows us to add support for modern features. For example,
     * 5-level page tables and more.
     */
    let _: OffsetPageTable<'static> = unsafe {
        let frame = FRAME_ALLOCATOR
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let phys_addr = frame.start_address();
        let virt_addr = PHYSICAL_MEMORY_OFFSET + phys_addr.as_u64();

        let page_table: *mut PageTable = virt_addr.as_mut_ptr();
        let page_table = &mut *page_table;

        OffsetPageTable::new(page_table, PHYSICAL_MEMORY_OFFSET)
    };

    Ok(offset_table)
}

/// Get a mutable reference to the active level 4 page table.
pub unsafe fn active_level_4_table() -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let physical = level_4_table_frame.start_address();
    let virtual_address = PHYSICAL_MEMORY_OFFSET + physical.as_u64();
    let page_table_ptr: *mut PageTable = virtual_address.as_mut_ptr();

    &mut *page_table_ptr
}

/// Identity maps a frame for a memory mapped device.
#[track_caller]
pub unsafe fn memory_map_device(
    offset_table: &mut OffsetPageTable,
    frame: PhysFrame,
) -> Result<UnmapGuard, MapToError<Size4KiB>> {
    let frame_type = FRAME_ALLOCATOR
        .get_frame_type(frame)
        .ok_or(MapToError::FrameAllocationFailed)?;

    let extra_flags = match frame_type {
        StivaleMemoryMapEntryType::Reserved => PageTableFlags::WRITABLE,
        _ => panic!(
            "Tried to memory map a device on a {:?} frame {:#X}",
            frame_type,
            frame.start_address()
        ),
    };

    let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));

    offset_table
        .identity_map(
            frame,
            PageTableFlags::PRESENT
                | PageTableFlags::NO_CACHE
                | PageTableFlags::WRITE_THROUGH
                | extra_flags,
            &mut FRAME_ALLOCATOR,
        )?
        .flush();

    Ok(UnmapGuard::new(page))
}
