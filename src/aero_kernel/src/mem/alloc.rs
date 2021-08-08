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

use core::alloc;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

use linked_list_allocator::{align_up, Heap};
use spin::Mutex;

use crate::mem::paging::*;
use crate::AERO_SYSTEM_ALLOCATOR;

use super::paging::FRAME_ALLOCATOR;
use super::AddressSpace;

const HEAP_START: usize = 0xfffffe8000000000;

pub struct LockedHeap(Mutex<Heap>);

impl LockedHeap {
    /// Creates a new uninitialized instance of the kernel
    /// global allocator.
    #[inline]
    pub const fn new_uninit() -> Self {
        Self(Mutex::new(Heap::empty()))
    }

    /// Allocate memory as described by the given `layout`.
    ///
    /// Returns a pointer to newly-allocated memory, or null to indicate
    /// allocation failure.
    unsafe fn allocate(&self, layout: alloc::Layout) -> Result<NonNull<u8>, ()> {
        let mut heap = self.0.lock();

        heap.allocate_first_fit(layout).or_else(|_| {
            let heap_top = heap.top();
            let size = align_up(layout.size(), 0x1000);

            let max_mem = HEAP_START + (128 * 1024 * 1024) as usize;

            // Check if our heap has not increased beyond the maximum allowed size.
            if heap_top + size > max_mem {
                panic!("The heap size has increased more then {}", max_mem)
            }

            // Else we just have to extend the heap.
            let mut address_space = AddressSpace::this();
            let mut offset_table = address_space.offset_page_table();

            let page_range = {
                let heap_start = VirtAddr::new(heap_top as _);
                let heap_end = heap_start + size - 1u64;

                let heap_start_page = Page::containing_address(heap_start);
                let heap_end_page = Page::containing_address(heap_end);

                Page::range_inclusive(heap_start_page, heap_end_page)
            };

            for page in page_range {
                let frame = unsafe {
                    FRAME_ALLOCATOR
                        .allocate_frame()
                        .expect("Failed to allocate frame to extend heap")
                };

                unsafe {
                    offset_table.map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        &mut FRAME_ALLOCATOR,
                    )
                }
                .expect("Failed to map frame to extend the heap")
                .flush();
            }

            heap.extend(size); // Now extend the heap.
            heap.allocate_first_fit(layout) // And try again.
        })
    }

    /// Initializes an empty heap.
    ///
    /// ## Safety
    /// This function should only be called once and the provided `start` address
    /// should be a valid address.
    unsafe fn init(&self, start: usize, size: usize) {
        self.0.lock().init(start, size);
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: alloc::Layout) -> *mut u8 {
        debug_assert!(layout.align().is_power_of_two());

        // Rounded up size is:
        //   size_rounded_up = (size + align - 1) & !(align - 1);
        //
        // We know from above that align != 0. If adding (align - 1)
        // does not overflow, then rounding up will be fine.
        //
        // Conversely, &-masking with !(align - 1) will subtract off
        // only low-order-bits. Thus if overflow occurs with the sum,
        // the &-mask cannot subtract enough to undo that overflow.
        //
        // Above implies that checking for summation overflow is both
        // necessary and sufficient.
        debug_assert!(layout.size() < usize::MAX - (layout.align() - 1));

        self.allocate(layout)
            .ok()
            .map_or(0 as *mut u8, |alloc| alloc.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .lock()
            .deallocate(NonNull::new_unchecked(ptr), layout)
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::Layout) -> ! {
    panic!(
        "Allocation error with size {} and layout {}",
        layout.size(),
        layout.align()
    )
}

/// Initialize the heap at the [HEAP_START].
pub fn init_heap(offset_table: &mut OffsetPageTable) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + 4096u64 - 1u64;

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
        AERO_SYSTEM_ALLOCATOR.init(HEAP_START, 4096);
    }

    Ok(())
}
