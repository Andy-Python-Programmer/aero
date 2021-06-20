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

use core::mem;

use super::{address::VirtualAddress, frame::*, page::*, *};

pub struct Mapper<'table> {
    level_4_table: &'table mut PageTable,
    page_table_walker: PageTableWalker,
}

impl<'table> Mapper<'table> {
    #[inline]
    pub fn new(
        level_4_table: &'table mut PageTable,
        physical_memory_offset: VirtualAddress,
    ) -> Self {
        Self {
            level_4_table,
            page_table_walker: PageTableWalker::new(physical_memory_offset),
        }
    }

    pub fn level_4_table(&mut self) -> &mut PageTable {
        &mut self.level_4_table
    }

    fn map_to_1gib<A>(
        &mut self,
        page: Page<Size1GiB>,
        frame: Frame<Size1GiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size1GiB>, MapperError<Size1GiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let p4 = &mut self.level_4_table;
        let p3 = self.page_table_walker.create_next_table(
            &mut p4[page.p4_index()],
            parent_table_flags,
            allocator,
        )?;

        if !p3[page.p3_index()].is_unused() {
            return Err(MapperError::PageAlreadyMapped(frame));
        }

        p3[page.p3_index()].set_address(frame.start_address(), flags | PageTableFlags::HUGE_PAGE);

        Ok(MapperFlush::new(page))
    }

    fn map_to_2mib<A>(
        &mut self,
        page: Page<Size2MiB>,
        frame: Frame<Size2MiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size2MiB>, MapperError<Size2MiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let p4 = &mut self.level_4_table;
        let p3 = self.page_table_walker.create_next_table(
            &mut p4[page.p4_index()],
            parent_table_flags,
            allocator,
        )?;
        let p2 = self.page_table_walker.create_next_table(
            &mut p3[page.p3_index()],
            parent_table_flags,
            allocator,
        )?;

        if !p2[page.p2_index()].is_unused() {
            return Err(MapperError::PageAlreadyMapped(frame));
        }

        p2[page.p2_index()].set_address(frame.start_address(), flags | PageTableFlags::HUGE_PAGE);

        Ok(MapperFlush::new(page))
    }

    fn map_to_4kib<A>(
        &mut self,
        page: Page<Size4KiB>,
        frame: Frame<Size4KiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size4KiB>, MapperError<Size4KiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let p4 = &mut self.level_4_table;
        let p3 = self.page_table_walker.create_next_table(
            &mut p4[page.p4_index()],
            parent_table_flags,
            allocator,
        )?;
        let p2 = self.page_table_walker.create_next_table(
            &mut p3[page.p3_index()],
            parent_table_flags,
            allocator,
        )?;
        let p1 = self.page_table_walker.create_next_table(
            &mut p2[page.p2_index()],
            parent_table_flags,
            allocator,
        )?;

        if !p1[page.p1_index()].is_unused() {
            return Err(MapperError::PageAlreadyMapped(frame));
        }

        p1[page.p1_index()].set_frame(frame, flags);

        Ok(MapperFlush::new(page))
    }
}

impl<'table> MapperMap<Size1GiB> for Mapper<'table> {
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<Size1GiB>,
        frame: Frame<Size1GiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<Size1GiB>, MapperError<Size1GiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        self.map_to_1gib(page, frame, flags, parent_table_flags, frame_allocator)
    }
}

impl<'table> MapperMap<Size2MiB> for Mapper<'table> {
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<Size2MiB>,
        frame: Frame<Size2MiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<Size2MiB>, MapperError<Size2MiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        self.map_to_2mib(page, frame, flags, parent_table_flags, frame_allocator)
    }
}

impl<'table> MapperMap<Size4KiB> for Mapper<'table> {
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<Size4KiB>,
        frame: Frame<Size4KiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<Size4KiB>, MapperError<Size4KiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        self.map_to_4kib(page, frame, flags, parent_table_flags, frame_allocator)
    }
}

pub trait MapperMap<S: PageSize> {
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<S>,
        frame: Frame<S>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<S>, MapperError<S>>
    where
        Self: Sized,
        A: FrameAllocator<Size4KiB> + ?Sized;

    #[inline]
    unsafe fn map_to<A>(
        &mut self,
        page: Page<S>,
        frame: Frame<S>,
        flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<S>, MapperError<S>>
    where
        Self: Sized,
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let parent_table_flags = flags
            & (PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE);

        self.map_to_with_table_flags(page, frame, flags, parent_table_flags, frame_allocator)
    }

    #[inline]
    unsafe fn identity_map<A>(
        &mut self,
        frame: Frame<S>,
        flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<S>, MapperError<S>>
    where
        Self: Sized,
        A: FrameAllocator<Size4KiB> + ?Sized,
        S: PageSize,
        Self: MapperMap<S>,
    {
        let page = Page::containing_address(VirtualAddress::new(frame.start_address().as_u64()));

        self.map_to(page, frame, flags, frame_allocator)
    }
}

#[must_use = "Page Table changes must be flushed or ignored."]
pub struct MapperFlush<S: PageSize>(Page<S>);

impl<S: PageSize> MapperFlush<S> {
    fn new(page: Page<S>) -> Self {
        Self(page)
    }

    pub fn flush(self) {
        unsafe {
            asm!("invlpg [{}]", in(reg) self.0.start_address().as_u64(), options(nostack));
        }

        mem::forget(self)
    }

    pub fn ignore(self) {
        mem::forget(self)
    }
}

impl<S: PageSize> Drop for MapperFlush<S> {
    fn drop(&mut self) {
        panic!("MapperFlush should never be dropped")
    }
}

#[derive(Debug)]
struct PageTableWalker(VirtualAddress);

impl PageTableWalker {
    #[inline(always)]
    fn new(physical_memory_offset: VirtualAddress) -> Self {
        Self(physical_memory_offset)
    }

    fn next_table_mut<'table>(&self, entry: &'table mut PageTableEntry) -> &'table mut PageTable {
        let page_table_ptr = self.frame_to_pointer(entry.frame());
        let page_table: &mut PageTable = unsafe { &mut *page_table_ptr };

        page_table
    }

    fn frame_to_pointer(&self, frame: Frame) -> *mut PageTable {
        let virt = self.0 + frame.start_address().as_u64();

        virt.as_mut_ptr()
    }

    fn create_next_table<'table, A, S: PageSize>(
        &self,
        entry: &'table mut PageTableEntry,
        insert_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<&'table mut PageTable, MapperError<S>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let created;

        if entry.is_unused() {
            if let Some(frame) = allocator.allocate_frame() {
                entry.set_frame(frame, insert_flags);

                created = true;
            } else {
                return Err(MapperError::FrameAllocationFailed);
            }
        } else {
            created = false;
        }

        let page_table = self.next_table_mut(entry);

        if created {
            cfg_if::cfg_if! {
                if #[cfg(target_arch = "aarch64")] {
                    // TODO: barrier::dsb(barrier::ISHST);
                }
            };

            page_table.zero();
        }

        Ok(page_table)
    }
}

/// This error is returned from `map_to` and similar methods.
#[derive(Debug)]
pub enum MapperError<S: PageSize> {
    /// An additional frame was needed for the mapping process, but the frame allocator
    /// returned `None`.
    FrameAllocationFailed,
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of an already mapped huge page.
    ParentEntryHugePage,
    /// The given page is already mapped to a physical frame.
    PageAlreadyMapped(Frame<S>),
}
