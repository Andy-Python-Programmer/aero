/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use crate::mem::paging::FRAME_ALLOCATOR;
use crate::prelude::*;

use x86_64::{
    structures::paging::{mapper::MapToError, *},
    VirtAddr,
};

pub struct Stack {
    stack_start: VirtAddr,
    stack_size: usize,
}

impl Stack {
    /// Allocates a user stack at the provided `stack_address` with the
    /// provided stack size.
    pub fn allocate_user(
        offset_table: &mut OffsetPageTable,
        stack_address: VirtAddr,
        stack_size: usize,
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
                        | PageTableFlags::USER_ACCESSIBLE,
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
