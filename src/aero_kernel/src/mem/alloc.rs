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

use linked_list_allocator::Heap;

use crate::utils::sync::Mutex;
use crate::AERO_SYSTEM_ALLOCATOR;

use super::paging::FRAME_ALLOCATOR;
use super::AddressSpace;
use crate::mem::paging::*;

const HEAP_MAX_SIZE: usize = 128 * 1024 * 1024; // 128 GiB
const HEAP_START: usize = 0xfffff80000000000;
const HEAP_END: usize = HEAP_START + HEAP_MAX_SIZE;

#[repr(C)]
struct SlabHeader {
    ptr: *mut Slab,
}

/// The slab is the primary unit of currency in the slab allocator.
struct Slab {
    size: usize,
    first_free: usize,
}

impl Slab {
    const fn new(size: usize) -> Self {
        Self {
            size,
            first_free: 0,
        }
    }

    fn init(&mut self) {
        unsafe {
            let frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR
                .allocate_frame()
                .expect("slab_init: failed to allocate frame");

            self.first_free = frame.start_address().as_u64() as usize;
            self.first_free += crate::PHYSICAL_MEMORY_OFFSET.as_u64() as usize;
        }

        let hdr_size = core::mem::size_of::<SlabHeader>() as u64;
        let aligned_hdr_size = align_up(hdr_size, self.size as u64) as usize;

        let avl_size = Size4KiB::SIZE as usize - aligned_hdr_size;

        let slab_ptr = unsafe { &mut *(self.first_free as *mut SlabHeader) };
        slab_ptr.ptr = self as *mut Slab;

        self.first_free += aligned_hdr_size;

        let arr_ptr = self.first_free as *mut usize;
        let array = unsafe { core::slice::from_raw_parts_mut(arr_ptr, avl_size) };

        // A slab is built by allocating a 4KiB page, placing the slab data at
        // the end, and dividing the rest into equal-size buffers:
        //
        // ------------------------------------------------------
        // | buffer | buffer | buffer | buffer | slab header
        // ------------------------------------------------------
        //                         one page
        let max = avl_size / self.size - 1;
        let fact = self.size / 8;

        for i in 0..max {
            unsafe {
                array[i * fact] = array.as_ptr().add((i + 1) * fact) as usize;
            }
        }

        array[max * fact] = 0;
    }

    fn alloc(&mut self) -> *mut u8 {
        if self.first_free == 0 {
            self.init();
        }

        let old_free = self.first_free as *mut usize;

        unsafe {
            self.first_free = *old_free;
        }

        old_free as *mut u8
    }

    fn dealloc(&mut self, ptr: *mut u8) {
        if ptr == core::ptr::null_mut() {
            panic!("dealloc: attempted to free a nullptr")
        }

        let new_head = ptr as *mut usize;

        unsafe {
            *new_head = self.first_free;
        }

        self.first_free = new_head as usize;
    }
}

struct ProtectedAllocator {
    slabs: [Slab; 10],
    linked_list_heap: Heap,
}

struct Allocator {
    inner: Mutex<ProtectedAllocator>,
}

impl Allocator {
    const fn new() -> Self {
        Self {
            inner: Mutex::new(ProtectedAllocator {
                slabs: [
                    Slab::new(8),
                    Slab::new(16),
                    Slab::new(24),
                    Slab::new(32),
                    Slab::new(48),
                    Slab::new(64),
                    Slab::new(128),
                    Slab::new(256),
                    Slab::new(512),
                    Slab::new(1024),
                ],

                linked_list_heap: Heap::empty(),
            }),
        }
    }

    fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.inner.lock_irq();

        let slab = inner
            .slabs
            .iter_mut()
            .find(|slab| slab.size >= layout.size());

        if let Some(slab) = slab {
            slab.alloc()
        } else {
            inner
                .linked_list_heap
                .allocate_first_fit(layout)
                .or_else(|_| {
                    let heap_top = inner.linked_list_heap.top();
                    let size = align_up(layout.size() as u64, 0x1000);

                    // Check if our heap has not increased beyond the maximum allowed size.
                    if heap_top + size as usize > HEAP_END {
                        panic!("the heap size has increased more then {:#x}", HEAP_END)
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

                    unsafe {
                        inner.linked_list_heap.extend(size as usize); // Now extend the heap.
                        inner.linked_list_heap.allocate_first_fit(layout) // And try again.
                    }
                })
                .expect("alloc: memory exhausted")
                .as_ptr()
        }
    }

    fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut inner = self.inner.lock_irq();
        let address = ptr as usize;

        if address >= HEAP_START && address < HEAP_END {
            unsafe {
                inner
                    .linked_list_heap
                    .deallocate(NonNull::new_unchecked(ptr), layout);
            }

            return;
        }

        let slab_header = (ptr as usize & !(0xfff)) as *mut SlabHeader;

        let slab_header = unsafe { &mut *slab_header };
        let slab = unsafe { &mut *slab_header.ptr };

        slab.dealloc(ptr);
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
    unsafe {
        let mut address_space = AddressSpace::this();
        let mut offset_table = address_space.offset_page_table();

        let frame: PhysFrame = FRAME_ALLOCATOR
            .allocate_frame()
            .expect("init_heap: failed to allocate frame for the linked list allocator");

        offset_table
            .map_to(
                Page::containing_address(VirtAddr::new(HEAP_START as _)),
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                &mut FRAME_ALLOCATOR,
            )
            .expect("init_heap: failed to initialize the heap")
            .flush();

        AERO_SYSTEM_ALLOCATOR
            .0
            .inner
            .lock_irq()
            .linked_list_heap
            .init(HEAP_START, Size4KiB::SIZE as usize);
    }

    #[cfg(feature = "kmemleak")]
    kmemleak::MEM_LEAK_CATCHER.init();
}
