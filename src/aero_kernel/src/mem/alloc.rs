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

use core::alloc::{self, GlobalAlloc, Layout};

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

use crate::AERO_SYSTEM_ALLOCATOR;

use super::paging::FRAME_ALLOCATOR;

pub const HEAP_START: usize = 0xfffffe8000000000;
pub const HEAP_SIZE: usize = 100 * 1024;

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::Layout) -> ! {
    panic!(
        "Allocation error with size {} and layout {}",
        layout.size(),
        layout.align()
    )
}

/// Initialize the heap.
pub fn init_heap(offset_table: &mut OffsetPageTable) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;

        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);

        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = unsafe {
            FRAME_ALLOCATOR
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?
        };

        unsafe {
            offset_table.map_to(
                page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                &mut FRAME_ALLOCATOR,
            )
        }?
        .flush();
    }

    unsafe {
        AERO_SYSTEM_ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

#[inline(always)]
#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    malloc_align(size, 8)
}

#[inline]
#[no_mangle]
pub extern "C" fn malloc_align(size: usize, align: usize) -> *mut u8 {
    let layout = Layout::from_size_align(size, align).expect("Invalid malloc layout");

    unsafe { AERO_SYSTEM_ALLOCATOR.alloc(layout) }
}
