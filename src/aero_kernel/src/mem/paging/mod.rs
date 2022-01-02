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

mod addr;
mod frame;
mod mapper;
mod page;
mod page_table;

pub use self::addr::*;
pub use self::frame::*;
pub use self::mapper::MapperFlush;
pub use self::mapper::*;
pub use self::page::*;
pub use self::page_table::*;

use stivale_boot::v2::StivaleMemoryMapTag;

pub use frame::LockedFrameAllocator;

use crate::arch::controlregs;
use crate::PHYSICAL_MEMORY_OFFSET;

pub static mut FRAME_ALLOCATOR: LockedFrameAllocator = LockedFrameAllocator::new_uninit();

bitflags::bitflags! {
    /// Describes an page fault error code.
    #[repr(transparent)]
    pub struct PageFaultErrorCode: u64 {
        /// If this flag is set, the page fault was caused by a page-protection violation,
        /// else the page fault was caused by a not-present page.
        const PROTECTION_VIOLATION = 1;

        /// If this flag is set, the memory access that caused the page fault was a write.
        /// Else the access that caused the page fault is a memory read. This bit does not
        /// necessarily indicate the cause of the page fault was a read or write violation.
        const CAUSED_BY_WRITE = 1 << 1;

        /// If this flag is set, an access in user mode (CPL=3) caused the page fault. Else
        /// an access in supervisor mode (CPL=0, 1, or 2) caused the page fault. This bit
        /// does not necessarily indicate the cause of the page fault was a privilege violation.
        const USER_MODE = 1 << 2;

        /// If this flag is set, the page fault is a result of the processor reading a 1 from
        /// a reserved field within a page-translation-table entry.
        const MALFORMED_TABLE = 1 << 3;

        /// If this flag is set, it indicates that the access that caused the page fault was an
        /// instruction fetch.
        const INSTRUCTION_FETCH = 1 << 4;
    }
}

/// Returns true if level 5 paging is supported by the CPU and is enabled in Cr4.
#[inline]
pub fn level_5_paging_enabled() -> bool {
    controlregs::read_cr4().contains(controlregs::Cr4Flags::L5_PAGING)
}

/// Initialize paging.
pub fn init(
    memory_regions: &'static StivaleMemoryMapTag,
) -> Result<OffsetPageTable<'static>, MapToError<Size4KiB>> {
    let memory_regions = unsafe {
        let addr = (memory_regions as *const StivaleMemoryMapTag) as u64;
        let new_addr = crate::PHYSICAL_MEMORY_OFFSET + addr;

        &*new_addr.as_mut_ptr::<StivaleMemoryMapTag>()
    };

    let active_level_4 = unsafe { active_level_4_table() };
    let offset_table = unsafe { OffsetPageTable::new(active_level_4, PHYSICAL_MEMORY_OFFSET) };

    unsafe {
        FRAME_ALLOCATOR.init(memory_regions);
    }

    Ok(offset_table)
}

/// Get a mutable reference to the active level 4 page table.
pub unsafe fn active_level_4_table() -> &'static mut PageTable {
    let (level_4_table_frame, _) = controlregs::read_cr3();

    let physical = level_4_table_frame.start_address();
    let virtual_address = PHYSICAL_MEMORY_OFFSET + physical.as_u64();
    let page_table_ptr: *mut PageTable = virtual_address.as_mut_ptr();

    &mut *page_table_ptr
}
