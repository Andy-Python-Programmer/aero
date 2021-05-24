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

use core::mem;

use x86_64::{structures::paging::*, PhysAddr, VirtAddr};

use crate::mem::paging::FRAME_ALLOCATOR;

pub(super) const XSDT_SIGNATURE: &str = "XSDT";
pub(super) const RSDT_SIGNATURE: &str = "RSDT";

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Sdt {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

impl Sdt {
    /// Get SDT from its address.
    pub unsafe fn from_address(address: u64, offset_table: &mut OffsetPageTable) -> &'static Self {
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(address));

        if offset_table.translate_page(page).is_err() {
            let frame = PhysFrame::containing_address(PhysAddr::new(page.start_address().as_u64()));

            offset_table
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                    &mut FRAME_ALLOCATOR,
                )
                .unwrap()
                .flush();
        }

        let sdt = &*(address as *const Self);

        // Map additional frames for the SDT is needed.
        let start_page: Page<Size4KiB> =
            Page::containing_address(VirtAddr::new(address + Size4KiB::SIZE));
        let end_page = Page::containing_address(VirtAddr::new(address + sdt.length as u64));

        for page in Page::range_inclusive(start_page, end_page) {
            if offset_table.translate_page(page).is_err() {
                let frame =
                    PhysFrame::containing_address(PhysAddr::new(page.start_address().as_u64()));
                offset_table
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                        &mut FRAME_ALLOCATOR,
                    )
                    .unwrap()
                    .flush();
            }
        }

        sdt
    }

    /// Get the address of this tables data.
    pub fn data_address(&self) -> usize {
        self as *const _ as usize + mem::size_of::<Self>()
    }

    /// Get the length of this tables data.
    pub fn data_len(&self) -> usize {
        let total_size = self.length as usize;
        let header_size = mem::size_of::<Self>();

        if total_size >= header_size {
            total_size - header_size
        } else {
            0
        }
    }

    /// Get the SDT's signature.
    pub fn get_signature(&self) -> &str {
        core::str::from_utf8(&self.signature).expect("Invalid UTF8 in SDT's signature")
    }

    #[inline(always)]
    pub(super) unsafe fn as_ptr<T>(&self) -> &'static T {
        &*(self as *const _ as *const T)
    }
}
