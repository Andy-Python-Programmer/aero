/*
 * Copyright (C) 2021-2023 The Aero Project Developers.
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
mod vmalloc;

use core::alloc::Layout;

use ::alloc::boxed::Box;

use crate::mem::paging::*;

use self::paging::{active_level_4_table, FRAME_ALLOCATOR};

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
            let virt_addr = phys_addr.as_hhdm_virt();

            phys_addr.as_vm_frame().unwrap().inc_ref_count();

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
        #[cfg(target_arch = "x86_64")]
        {
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

        #[cfg(target_arch = "aarch64")]
        unimplemented!()
    }

    pub fn switch(&self) {
        #[cfg(target_arch = "x86_64")]
        {
            let cr3 = self.cr3().start_address().as_u64();

            unsafe {
                asm!("mov cr3, {}", in(reg) cr3, options(nostack)); // Load the new address space
            }
        }

        #[cfg(target_arch = "aarch64")]
        unimplemented!()
    }

    /// Returns a reference to the page table frame allocated for this address
    /// space.
    pub fn cr3(&self) -> PhysFrame {
        self.cr3
    }

    /// Returns a mutable reference to the page table allocated for this
    /// address space.
    pub fn page_table(&mut self) -> &'static mut PageTable {
        unsafe { &mut *(self.cr3.start_address().as_hhdm_virt().as_mut_ptr()) }
    }

    /// Returns a mutable reference to the mapper pointing to the page table
    /// allocated for this address space.
    pub fn offset_page_table(&mut self) -> OffsetPageTable {
        unsafe { OffsetPageTable::new(self.page_table(), crate::PHYSICAL_MEMORY_OFFSET) }
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
