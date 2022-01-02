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

pub mod alloc;
pub mod paging;
pub mod pti;

use core::alloc::Layout;

use ::alloc::boxed::Box;

use crate::mem::paging::*;

use self::paging::{active_level_4_table, FRAME_ALLOCATOR};
use crate::PHYSICAL_MEMORY_OFFSET;

/// Structure representing a *virtual* address space. The address space
/// contains a reference of the page table allocated for this address space.
pub struct AddressSpace {
    cr3: PhysFrame,
}

impl AddressSpace {
    /// Allocates a new *virtual* address space.
    pub fn new() -> Result<Self, MapToError<Size4KiB>> {
        let cr3 = unsafe {
            let frame = FRAME_ALLOCATOR
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;

            let phys_addr = frame.start_address();
            phys_addr.as_vm_frame().unwrap().inc_ref_count();

            let virt_addr = PHYSICAL_MEMORY_OFFSET + phys_addr.as_u64();

            let page_table: *mut PageTable = virt_addr.as_mut_ptr();
            let page_table = &mut *page_table;

            let current_table = active_level_4_table();

            // Zero out the page table entries from the range 0..256.
            for i in 0..256 {
                page_table[i].set_unused();
            }

            // Map the higher half of the kernel's address space into this address
            // space.
            for i in 256..512 {
                page_table[i] = current_table[i].clone();
            }

            frame
        };

        Ok(Self { cr3 })
    }

    /// Returns the current active address space.
    pub fn this() -> Self {
        let cr3 = {
            // Get the value of the Cr3 register.
            let value: u64;

            unsafe {
                asm!("mov {}, cr3", out(reg) value, options(nomem));
            }

            let addr = PhysAddr::new(value & 0x_000f_ffff_ffff_f000);
            let frame = PhysFrame::containing_address(addr);

            frame
        };

        Self { cr3 }
    }

    pub fn switch(&self) {
        let cr3 = self.cr3().start_address().as_u64();

        unsafe {
            asm!("mov cr3, {}", in(reg) cr3, options(nostack)); // Load the new address space
        }
    }

    /// Returns a reference to the page table frame allocated for this address
    /// space.
    pub fn cr3(&self) -> PhysFrame {
        self.cr3
    }

    /// Returns a mutable reference to the page table allocated for this
    /// address space.
    pub fn page_table(&mut self) -> &'static mut PageTable {
        unsafe {
            let phys_addr = self.cr3.start_address();
            let virt_addr = PHYSICAL_MEMORY_OFFSET + phys_addr.as_u64();
            let page_table_ptr: *mut PageTable = virt_addr.as_mut_ptr();

            &mut *page_table_ptr
        }
    }

    /// Returns a mutable refernce to the mapper pointing to the page table
    /// allocated for this address space.
    pub fn offset_page_table(&mut self) -> OffsetPageTable {
        unsafe { OffsetPageTable::new(self.page_table(), PHYSICAL_MEMORY_OFFSET) }
    }
}

pub fn alloc_boxed_buffer<T>(size: usize) -> Box<[T]> {
    if size == 0 {
        return <Box<[T]>>::default();
    }

    let layout = unsafe { Layout::from_size_align_unchecked(size, 8) };
    let ptr = unsafe { ::alloc::alloc::alloc_zeroed(layout) as *mut T };
    let slice_ptr = core::ptr::slice_from_raw_parts_mut(ptr, size);

    unsafe { Box::from_raw(slice_ptr) }
}

/// Creates a Rust string from the provided C string.
///
/// ## Safety
/// - The provided pointer must be valid.
/// - The provided pointer must point to a null-terminated C string.
/// - The returned lifetime is not guaranteed to be the actual lifetime
/// of `ptr`.
/// - It is not guaranteed that the memory pointed by `ptr` won’t change
/// before the Rust string has been destroyed.
pub unsafe fn c_str_as_str<'cstring>(ptr: *const u8) -> &'cstring str {
    let length = c_strlen(ptr);
    let slice = core::slice::from_raw_parts(ptr, length);

    core::str::from_utf8_unchecked(slice)
}

/// Determines the provided of the given C string.
///
/// ## Safety
/// - The provided pointer must be valid.
/// - The provided pointer must point to a null-terminated C string.
pub unsafe fn c_strlen(mut ptr: *const u8) -> usize {
    let mut length = 0;

    while *ptr != 0 {
        ptr = ptr.offset(1);
        length += 1;
    }

    length
}
