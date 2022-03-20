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

use aero_syscall::*;
use core::ptr::NonNull;
use linked_list_allocator::LockedHeap;
use spin::Once;

#[global_allocator]
static ALLOCATOR: WrappedHeap = WrappedHeap::new();

struct WrappedHeap {
    heap: Once<LockedHeap>,
}

impl WrappedHeap {
    pub const fn new() -> Self {
        Self { heap: Once::new() }
    }
}

unsafe impl core::alloc::GlobalAlloc for WrappedHeap {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut heap = self.heap.call_once(init_heap).lock();

        match heap.allocate_first_fit(layout) {
            Ok(result) => result.as_ptr(),
            Err(_) => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let mut heap = self.heap.call_once(init_heap).lock();

        match NonNull::new(ptr) {
            Some(ptr) => heap.deallocate(ptr, layout),
            None => panic!("Attempted to free a null pointer"),
        }
    }
}

fn init_heap() -> LockedHeap {
    const HEAP_SIZE: usize = 1024 * 1024 * 8; // 8MiB

    let allocator = LockedHeap::empty();
    let heap_start = sys_mmap(
        0,
        HEAP_SIZE,
        MMapProt::PROT_READ | MMapProt::PROT_WRITE,
        MMapFlags::MAP_ANONYOMUS | MMapFlags::MAP_PRIVATE,
        -1isize as usize,
        0,
    )
    .expect("Failed to allocate virtual memory for heap");

    unsafe {
        allocator.lock().init(heap_start, HEAP_SIZE);
    }

    allocator
}
