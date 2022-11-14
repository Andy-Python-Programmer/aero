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

use super::paging::FRAME_ALLOCATOR;
use super::vmalloc;
use crate::mem::paging::*;

struct Allocator {}

impl Allocator {
    const fn new() -> Self {
        Self {}
    }

    fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = align_up(layout.size() as _, layout.align() as _);

        if size <= Size4KiB::SIZE {
            let frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR.allocate_frame().unwrap();
            frame.start_address().as_hhdm_virt().as_mut_ptr()
        } else if size <= Size2MiB::SIZE {
            let frame: PhysFrame<Size2MiB> = FRAME_ALLOCATOR.allocate_frame().unwrap();
            frame.start_address().as_hhdm_virt().as_mut_ptr()
        } else {
            let size = align_up(size, Size4KiB::SIZE) / Size4KiB::SIZE;

            vmalloc::get_vmalloc()
                .alloc(size as usize)
                .map(|addr| addr.as_mut_ptr::<u8>())
                .unwrap_or(core::ptr::null_mut())
        }
    }

    fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let address = VirtAddr::new(ptr as u64);

        if address >= vmalloc::VMALLOC_START && address < vmalloc::VMALLOC_END {
            vmalloc::get_vmalloc().dealloc(address, layout.size() / Size4KiB::SIZE as usize);
            return;
        }

        // TODO: free the slab.
    }
}

pub struct LockedHeap(Allocator);

impl LockedHeap {
    /// Creates a new uninitialized instance of the kernel
    /// global allocator.
    #[inline]
    pub const fn new_uninit() -> Self {
        Self(Allocator::new())
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

        // SAFETY: We we need to be careful to not cause a deadlock as the interrupt
        // handlers utilize the heap and might interrupt an in-progress allocation. So, we
        // lock the interrupts during the allocation.
        let ptr = self.0.alloc(layout);

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

        self.0.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        // SAFETY: the caller must ensure that `new_layout` is greater than zero.
        let new_ptr = self.alloc(new_layout);

        // NOTE: It is fine to pass a NULL pointer to `realloc` so, we need to check for that.
        if !new_ptr.is_null() && !ptr.is_null() {
            // SAFETY: the previously allocated block cannot overlap the newly allocated block.
            // The safety contract for `dealloc` must be upheld by the caller.
            core::ptr::copy_nonoverlapping(ptr, new_ptr, core::cmp::min(layout.size(), new_size));
            self.dealloc(ptr, layout);
        }

        new_ptr
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
pub fn init_heap() {
    vmalloc::init();

    #[cfg(feature = "kmemleak")]
    kmemleak::MEM_LEAK_CATCHER.init();
}
