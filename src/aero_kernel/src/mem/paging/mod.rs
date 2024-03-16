// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

mod addr;
mod frame;
mod mapper;
mod page;
mod page_table;

pub use self::addr::*;
pub use self::frame::*;
pub use self::mapper::*;
pub use self::page::*;
pub use self::page_table::*;

pub use frame::LockedFrameAllocator;

use crate::PHYSICAL_MEMORY_OFFSET;

pub static FRAME_ALLOCATOR: LockedFrameAllocator = LockedFrameAllocator::new_uninit();

bitflags::bitflags! {
    /// Describes an page fault error code.
    #[derive(Debug, Copy, Clone)]
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
#[cfg(target_arch = "x86_64")]
pub fn level_5_paging_enabled() -> bool {
    use crate::arch::controlregs;
    controlregs::read_cr4().contains(controlregs::Cr4Flags::L5_PAGING)
}

#[cfg(target_arch = "aarch64")]
pub const fn level_5_paging_enabled() -> bool {
    false
}

/// Initialize paging.
pub fn init(
    mmap_resp: &mut limine::response::MemoryMapResponse,
) -> Result<OffsetPageTable<'static>, MapToError<Size4KiB>> {
    let active_level_4 = unsafe { active_level_4_table() };
    let offset_table = unsafe { OffsetPageTable::new(active_level_4, PHYSICAL_MEMORY_OFFSET) };

    FRAME_ALLOCATOR.init(mmap_resp);
    Ok(offset_table)
}

/// Get a mutable reference to the active level 4 page table.
#[cfg(target_arch = "x86_64")]
pub unsafe fn active_level_4_table() -> &'static mut PageTable {
    use crate::arch::controlregs;

    let (level_4_table_frame, _) = controlregs::read_cr3();

    let virtual_address = level_4_table_frame.start_address().as_hhdm_virt();
    let page_table_ptr: *mut PageTable = virtual_address.as_mut_ptr();

    &mut *page_table_ptr
}

#[cfg(target_arch = "aarch64")]
pub unsafe fn active_level_4_table() -> &'static mut PageTable {
    unimplemented!()
}
