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

use crate::mem::paging::*;
use crate::prelude::*;

pub struct Stack {
    stack_start: VirtAddr,
    stack_size: usize,
}

impl Stack {
    /// Allocates a new stack at the provided stack address and the provided
    /// stack size.
    pub fn new_pinned(
        offset_table: &mut OffsetPageTable,
        stack_address: VirtAddr,
        stack_size: usize,
        flags: PageTableFlags,
    ) -> Result<Self, MapToError<Size4KiB>> {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "x86_64")] {
                let start_addr = stack_address - (stack_size - 1);
                let end_addr = stack_address;
            } else {
                let start_addr = stack_address;
                let end_addr = start_addr + (stack_size - 1);
            }
        }

        let page_range = {
            let start_page: Page = Page::containing_address(start_addr);
            let end_page = Page::containing_address(end_addr);

            Page::range_inclusive(start_page, end_page)
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
                    PageTableFlags::PRESENT
                        | PageTableFlags::NO_EXECUTE
                        | PageTableFlags::WRITABLE
                        | flags,
                    &mut FRAME_ALLOCATOR,
                )
            }?
            .flush();
        }

        unsafe {
            memset(start_addr.as_mut_ptr(), 0x00, stack_size);
        }

        Ok(Self {
            stack_start: start_addr,
            stack_size,
        })
    }

    /// Allocates a user stack at the provided `stack_address` with the
    /// provided stack size.
    pub fn new_user_pinned(
        offset_table: &mut OffsetPageTable,
        stack_address: VirtAddr,
        stack_size: usize,
    ) -> Result<Self, MapToError<Size4KiB>> {
        Self::new_pinned(
            offset_table,
            stack_address,
            stack_size,
            PageTableFlags::USER_ACCESSIBLE,
        )
    }

    pub fn stack_top(&self) -> VirtAddr {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "x86_64")] {
                self.stack_start + self.stack_size
            } else {
                self.stack_start
            }
        }
    }
}

pub struct StackHelper<'a> {
    ptr: &'a mut u64,
}

impl<'a> StackHelper<'a> {
    pub fn new(ptr: &'a mut u64) -> StackHelper<'a> {
        StackHelper::<'a> { ptr }
    }

    pub fn skip_by(&mut self, by: u64) {
        *self.ptr -= by;
    }

    pub unsafe fn offset<T: Sized>(&mut self) -> &mut T {
        self.skip_by(core::mem::size_of::<T>() as u64);

        &mut *(*self.ptr as *mut T)
    }
}
