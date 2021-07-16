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

pub mod alloc;
pub mod paging;
pub mod pti;

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

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;

    while i < n {
        *dest.offset(i as isize) = *src.offset(i as isize);
        i += 1;
    }

    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if src < dest as *const u8 {
        let mut i = n;

        // Copy from the end.
        while i != 0 {
            i -= 1;
            *dest.offset(i as isize) = *src.offset(i as isize);
        }
    } else {
        let mut i = 0;

        // Copy from the start.
        while i < n {
            *dest.offset(i as isize) = *src.offset(i as isize);
            i += 1;
        }
    }

    return dest;
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;

    while i < n {
        *s.offset(i as isize) = c as u8;
        i += 1;
    }

    return s;
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    let mut i = 0;

    while i < n {
        let a = *s1.offset(i as isize);
        let b = *s2.offset(i as isize);

        if a != b {
            return a as i32 - b as i32;
        }

        i += 1;
    }

    return 0;
}
