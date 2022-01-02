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

use core::alloc;
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

use crate::utils::sync::Mutex;
use linked_list_allocator::{align_up, Heap};

use crate::mem::paging::*;
use crate::AERO_SYSTEM_ALLOCATOR;

use super::paging::FRAME_ALLOCATOR;
use super::AddressSpace;

const HEAP_MAX_SIZE: usize = 128 * 1024 * 1024; // 128 GiB
const HEAP_START: usize = 0xfffff80000000000;
const HEAP_END: usize = HEAP_START + HEAP_MAX_SIZE;

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
        // SAFETY: We we need to be careful to not cause a deadlock as the interrupt
        // handlers utilize the heap and might interrupt an in-progress allocation. So, we
        // lock the interrupts during the allocation.
        let mut heap = self.0.lock_irq();

        heap.allocate_first_fit(layout).or_else(|_| {
            let heap_top = heap.top();
            let size = align_up(layout.size(), 0x1000);

            // Check if our heap has not increased beyond the maximum allowed size.
            if heap_top + size > HEAP_END {
                panic!("The heap size has increased more then {:#x}", HEAP_END)
            }

            // Else we just have to extend the heap.
            let mut address_space = AddressSpace::this();
            let mut offset_table = address_space.offset_page_table();

            let page_range = {
                let heap_start = VirtAddr::new(heap_top as _);
                let heap_end = heap_start + size - 1u64;

                let heap_start_page: Page = Page::containing_address(heap_start);
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

#[cfg(feature = "kmemleak")]
mod kmemleak {
    use core::alloc::Layout;
    use core::sync::atomic::{AtomicBool, Ordering};

    use crate::utils::sync::Mutex;
    use hashbrown::HashMap;
    use spin::Once;

    pub static MEM_LEAK_CATCHER: MemoryLeakCatcher = MemoryLeakCatcher::new_uninit();

    pub struct MemoryLeakCatcher {
        alloc: Once<Mutex<HashMap<usize, Layout>>>,
        initialized: AtomicBool,
    }

    impl MemoryLeakCatcher {
        /// Creates a new uninitialized instance of the kernel memory
        /// leak catcher.
        const fn new_uninit() -> Self {
            Self {
                alloc: Once::new(),
                initialized: AtomicBool::new(false),
            }
        }

        fn disable(&self) {
            self.initialized.store(false, Ordering::SeqCst);
        }

        fn enable(&self) {
            self.initialized.store(true, Ordering::SeqCst);
        }

        fn is_initialized(&self) -> bool {
            self.initialized.load(Ordering::SeqCst)
        }

        pub fn init(&self) {
            self.alloc.call_once(|| Mutex::new(HashMap::new()));
            self.enable();
        }

        pub fn track_caller(&self, ptr: *mut u8, layout: Layout) {
            let init = self.is_initialized();

            if !init {
                return;
            }

            self.disable();

            self.alloc
                .get()
                .expect("track_caller: leak catcher not initialized")
                .lock()
                .insert(ptr as usize, layout);

            self.enable();
        }

        pub fn unref(&self, ptr: *mut u8) {
            let init = self.is_initialized();

            if !init {
                return;
            }

            self.disable();

            let mut alloc = self
                .alloc
                .get()
                .expect("unref: leak catcher not initialized")
                .lock();

            let double_free = alloc.remove(&(ptr as usize));

            // If the allocation was not found, then we have a double free. Oh well!
            if double_free.is_none() {
                panic!(
                    "attempted to double-free pointer at address: {:#x}",
                    ptr as usize
                );
            }

            self.enable();
        }
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

        let ptr = self.allocate(layout).unwrap().as_ptr();

        #[cfg(feature = "kmemleak")]
        kmemleak::MEM_LEAK_CATCHER.track_caller(ptr, layout);

        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: We we need to be careful to not cause a deadlock as the interrupt
        // handlers utilize the heap and might interrupt an in-progress de-allocation. So, we
        // lock the interrupts during the de-allocation.
        #[cfg(feature = "kmemleak")]
        kmemleak::MEM_LEAK_CATCHER.unref(ptr);

        self.0
            .lock_irq()
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
    let frame: PhysFrame = unsafe {
        FRAME_ALLOCATOR
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?
    };

    unsafe {
        offset_table.map_to(
            Page::containing_address(VirtAddr::new(HEAP_START as _)),
            frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            &mut FRAME_ALLOCATOR,
        )
    }?
    .flush();

    unsafe {
        AERO_SYSTEM_ALLOCATOR.init(HEAP_START, 4096);
    }

    #[cfg(feature = "kmemleak")]
    kmemleak::MEM_LEAK_CATCHER.init();

    Ok(())
}
