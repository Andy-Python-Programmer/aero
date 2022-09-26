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

//! Due to internal-fragmentation in the buddy frame allocator, we cannot allocate large
//! amount of contiguous physical memory. We instead use [`vmalloc`] to allocate virtually
//! contiguous memory. The allocator uses a red-black tree to keep track of the free memory
//! so we can allocate and free memory efficiently.
//!
//! An area is reserved for [`vmalloc`] in the kernel address space, starting
//! at [`VMALLOC_VIRT_START`] and ending at [`VMALLOC_VIRT_END`].

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use intrusive_collections::*;
use spin::Once;

use crate::utils::sync::{Mutex, MutexGuard};

use super::paging::*;
use super::AddressSpace;

pub(super) const VMALLOC_MAX_SIZE: usize = 128 * 1024 * 1024; // 128 GiB
pub(super) const VMALLOC_START: VirtAddr = VirtAddr::new(0xfffff80000000000);
pub(super) const VMALLOC_END: VirtAddr =
    VirtAddr::new(0xfffff80000000000 + VMALLOC_MAX_SIZE as u64);

static VMALLOC: Once<Mutex<Vmalloc>> = Once::new();

struct VmallocAreaProtected {
    addr: VirtAddr,
    size: usize,
}

impl VmallocAreaProtected {
    fn new(addr: VirtAddr, size: usize) -> Self {
        Self { addr, size }
    }
}

struct VmallocArea {
    // NOTE: Since there are equal amount of read and write operations we are going to
    // protect the data using a [`Mutex`].
    protected: Mutex<VmallocAreaProtected>,
    link: RBTreeLink,
}

impl VmallocArea {
    fn new(addr: VirtAddr, size: usize) -> Self {
        Self {
            protected: Mutex::new(VmallocAreaProtected::new(addr, size)),
            link: Default::default(),
        }
    }
}

impl<'a> KeyAdapter<'a> for VmallocAreaAdaptor {
    type Key = usize;

    fn get_key(&self, this: &'a VmallocArea) -> Self::Key {
        // NOTE: We use the size of the vmalloc area as the key for the red-black tree
        // so when we are allocating or deallocating memory we can find a large enough, free
        // vmalloc area efficiently.
        this.protected.lock().size
    }
}

intrusive_collections::intrusive_adapter!(VmallocAreaAdaptor = Box<VmallocArea>: VmallocArea { link: RBTreeLink });

pub(super) struct Vmalloc {
    free_list: VecDeque<VmallocArea>,
}

impl Vmalloc {
    fn new() -> Self {
        let mut this = Self {
            free_list: VecDeque::new(),
        };

        this.free_list
            .push_back(VmallocArea::new(VMALLOC_START, VMALLOC_MAX_SIZE));

        this
    }

    pub(super) fn alloc(&mut self, npages: usize) -> Option<VirtAddr> {
        // +1: area for the guard page.
        let size_bytes = (npages + 1) * Size4KiB::SIZE as usize;

        let (i, area) = self
            .free_list
            .iter()
            .enumerate()
            .find(|(_, area)| area.protected.lock().size >= size_bytes)?;

        let mut area_p = area.protected.lock();
        let address = area_p.addr.clone();

        if area_p.size > size_bytes {
            area_p.addr = area_p.addr + size_bytes;
            area_p.size -= size_bytes;
        } else {
            // NOTE: the area is has exactly the requested size, so we can remove it
            // from the free list.
            core::mem::drop(area_p); // unlock

            self.free_list.remove(i);
        }

        // subtract the size of the guard page since we are not required to allocate
        // a frame for that area.
        let size_bytes = size_bytes - Size4KiB::SIZE as usize;

        let mut address_space = AddressSpace::this();
        let mut offset_table = address_space.offset_page_table();

        let page_range = {
            let start_page: Page = Page::containing_address(address);
            let end_page = Page::containing_address(address + size_bytes);

            Page::range(start_page, end_page)
        };

        // map the pages at the allocated address.
        for page in page_range {
            let frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR
                .allocate_frame()
                .expect("vmalloc: physical memory exhausted");

            unsafe {
                offset_table.map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                )
            }
            .unwrap()
            .flush();
        }

        Some(address)
    }

    pub(super) fn dealloc(&mut self, addr: VirtAddr, npages: usize) {
        // +1: area for the guard page.
        let size = (npages + 1) * Size4KiB::SIZE as usize;

        // check if this block can be merged into another block.
        let merge = self
            .free_list
            .iter()
            .find(|area| addr + size == area.protected.lock().addr);

        if let Some(merge) = merge {
            let mut merge = merge.protected.lock();

            merge.addr = addr;
            merge.size += size;
        } else {
            // We add it to the back of the free list since, its more likely
            // to find larger free areas in the front of the list.
            self.free_list.push_back(VmallocArea::new(addr, size));
        }

        // subtract the size of the guard page since its not mapped.
        let size = size - Size4KiB::SIZE as usize;

        let mut address_space = AddressSpace::this();
        let mut offset_table = address_space.offset_page_table();

        let page_range = {
            let start_page: Page = Page::containing_address(addr);
            let end_page = Page::containing_address(addr + size);

            Page::range(start_page, end_page)
        };

        for page in page_range {
            // unmap the page at the address which in turn will deallocate
            // the frame (refcnt == 0).
            offset_table.unmap(page).unwrap().1.flush();
        }
    }
}

pub fn init() {
    VMALLOC.call_once(|| Mutex::new(Vmalloc::new()));
}

/// ## Panics
/// * If the `vmalloc` allocator is not initialized.
pub(super) fn get_vmalloc() -> MutexGuard<'static, Vmalloc> {
    VMALLOC
        .get()
        .expect("get_vmalloc: not initialized")
        .lock_irq()
}
