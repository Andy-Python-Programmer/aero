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

// Some code borrowed from the x86_64 crate (MIT + Apache) and add support for 5-level paging
// and some kernel specific features that cannot be directly done in the crate itself.

use core::ops::Range;

use crate::mem::AddressSpace;

use super::{
    addr::{PhysAddr, VirtAddr},
    page::PhysFrame,
    page::{AddressNotAligned, Page, PageSize, Size1GiB, Size2MiB, Size4KiB},
    page_table::{FrameError, PageTable, PageTableEntry, PageTableFlags},
    FRAME_ALLOCATOR,
};

/// A trait for types that can allocate a frame of memory.
///
/// This trait is unsafe to implement because the implementer must guarantee that
/// the `allocate_frame` method returns only unique unused frames.
pub unsafe trait FrameAllocator<S: PageSize> {
    /// Allocate a frame of the appropriate size and return it if possible.
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>>;
    fn deallocate_frame(&mut self, frame: PhysFrame<S>);
}

/// An empty convencience trait that requires the `Mapper` trait for all page sizes.
pub trait MapperAllSizes: Mapper<Size4KiB> + Mapper<Size2MiB> + Mapper<Size1GiB> {}

impl<T> MapperAllSizes for T where T: Mapper<Size4KiB> + Mapper<Size2MiB> + Mapper<Size1GiB> {}

/// Provides methods for translating virtual addresses.
pub trait Translate {
    /// Return the frame that the given virtual address is mapped to and the offset within that
    /// frame.
    ///
    /// If the given address has a valid mapping, the mapped frame and the offset within that
    /// frame is returned. Otherwise an error value is returned.
    ///
    /// This function works with huge pages of all sizes.
    fn translate(&self, addr: VirtAddr) -> TranslateResult;

    /// Translates the given virtual address to the physical address that it maps to.
    ///
    /// Returns `None` if there is no valid mapping for the given address.
    ///
    /// This is a convenience method. For more information about a mapping see the
    /// [`translate`](Translate::translate) method.
    #[inline]
    fn translate_addr(&self, addr: VirtAddr) -> Option<PhysAddr> {
        match self.translate(addr) {
            TranslateResult::NotMapped | TranslateResult::InvalidFrameAddress(_) => None,
            TranslateResult::Mapped { frame, offset, .. } => Some(frame.start_address() + offset),
        }
    }
}

/// The return value of the [`Translate::translate`] function.
///
/// If the given address has a valid mapping, a `Frame4KiB`, `Frame2MiB`, or `Frame1GiB` variant
/// is returned, depending on the size of the mapped page. The remaining variants indicate errors.
#[derive(Debug)]
pub enum TranslateResult {
    /// The virtual address is mapped to a physical frame.
    Mapped {
        /// The mapped frame.
        frame: MappedFrame,
        /// The offset whithin the mapped frame.
        offset: u64,
        /// The entry flags in the lowest-level page table.
        ///
        /// Flags of higher-level page table entries are not included here, but they can still
        /// affect the effective flags for an address, for example when the WRITABLE flag is not
        /// set for a level 3 entry.
        flags: PageTableFlags,
    },
    /// The given virtual address is not mapped to a physical frame.
    NotMapped,
    /// The page table entry for the given virtual address points to an invalid physical address.
    InvalidFrameAddress(PhysAddr),
}

/// Represents a physical frame mapped in a page table.
#[derive(Debug)]
pub enum MappedFrame {
    /// The virtual address is mapped to a 4KiB frame.
    Size4KiB(PhysFrame<Size4KiB>),
    /// The virtual address is mapped to a "large" 2MiB frame.
    Size2MiB(PhysFrame<Size2MiB>),
    /// The virtual address is mapped to a "huge" 1GiB frame.
    Size1GiB(PhysFrame<Size1GiB>),
}

impl MappedFrame {
    /// Returns the start address of the frame.
    pub const fn start_address(&self) -> PhysAddr {
        match self {
            MappedFrame::Size4KiB(frame) => frame.start_address,
            MappedFrame::Size2MiB(frame) => frame.start_address,
            MappedFrame::Size1GiB(frame) => frame.start_address,
        }
    }

    /// Returns the size the frame (4KB, 2MB or 1GB).
    #[allow(unused)]
    pub const fn size(&self) -> u64 {
        match self {
            MappedFrame::Size4KiB(_) => Size4KiB::SIZE,
            MappedFrame::Size2MiB(_) => Size2MiB::SIZE,
            MappedFrame::Size1GiB(_) => Size1GiB::SIZE,
        }
    }
}

/// A trait for common page table operations on pages of size `S`.
pub trait Mapper<S: PageSize> {
    /// Creates a new mapping in the page table.
    ///
    /// This function might need additional physical frames to create new page tables. These
    /// frames are allocated from the `allocator` argument. At most three frames are required.
    ///
    /// Parent page table entries are automatically updated with `PRESENT | WRITABLE | USER_ACCESSIBLE`
    /// if present in the `PageTableFlags`. Depending on the used mapper implementation
    /// the `PRESENT` and `WRITABLE` flags might be set for parent tables,
    /// even if they are not set in `PageTableFlags`.
    ///
    /// The `map_to_with_table_flags` method gives explicit control over the parent page table flags.
    ///
    /// ## Safety
    ///
    /// Creating page table mappings is a fundamentally unsafe operation because
    /// there are various ways to break memory safety through it. For example,
    /// re-mapping an in-use page to a different frame changes and invalidates
    /// all values stored in that page, resulting in undefined behavior on the
    /// next use.
    ///
    /// The caller must ensure that no undefined behavior or memory safety
    /// violations can occur through the new mapping. Among other things, the
    /// caller must prevent the following:
    ///
    /// - Aliasing of `&mut` references, i.e. two `&mut` references that point to
    ///   the same physical address. This is undefined behavior in Rust.
    ///     - This can be ensured by mapping each page to an individual physical
    ///       frame that is not mapped anywhere else.
    /// - Creating uninitalized or invalid values: Rust requires that all values
    ///   have a correct memory layout. For example, a `bool` must be either a 0
    ///   or a 1 in memory, but not a 3 or 4. An exception is the `MaybeUninit`
    ///   wrapper type, which abstracts over possibly uninitialized memory.
    ///     - This is only a problem when re-mapping pages to different physical
    ///       frames. Mapping a page that is not in use yet is fine.
    ///
    /// Special care must be taken when sharing pages with other address spaces,
    /// e.g. by setting the `GLOBAL` flag. For example, a global mapping must be
    /// the same in all address spaces, otherwise undefined behavior can occur
    /// because of TLB races. It's worth noting that all the above requirements
    /// also apply to shared mappings, including the aliasing requirements.
    #[inline]
    unsafe fn map_to<A>(
        &mut self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<S>, MapToError<S>>
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

    /// Creates a new mapping in the page table.
    ///
    /// This function might need additional physical frames to create new page tables. These
    /// frames are allocated from the `allocator` argument. At most three frames are required.
    ///
    /// The flags of the parent table(s) can be explicitly specified. Those flags are used for
    /// newly created table entries, and for existing entries the flags are added.
    ///
    /// Depending on the used mapper implementation, the `PRESENT` and `WRITABLE` flags might
    /// be set for parent tables, even if they are not specified in `parent_table_flags`.
    ///
    /// ## Safety
    ///
    /// Creating page table mappings is a fundamentally unsafe operation because
    /// there are various ways to break memory safety through it. For example,
    /// re-mapping an in-use page to a different frame changes and invalidates
    /// all values stored in that page, resulting in undefined behavior on the
    /// next use.
    ///
    /// The caller must ensure that no undefined behavior or memory safety
    /// violations can occur through the new mapping. Among other things, the
    /// caller must prevent the following:
    ///
    /// - Aliasing of `&mut` references, i.e. two `&mut` references that point to
    ///   the same physical address. This is undefined behavior in Rust.
    ///     - This can be ensured by mapping each page to an individual physical
    ///       frame that is not mapped anywhere else.
    /// - Creating uninitalized or invalid values: Rust requires that all values
    ///   have a correct memory layout. For example, a `bool` must be either a 0
    ///   or a 1 in memory, but not a 3 or 4. An exception is the `MaybeUninit`
    ///   wrapper type, which abstracts over possibly uninitialized memory.
    ///     - This is only a problem when re-mapping pages to different physical
    ///       frames. Mapping a page that is not in use yet is fine.
    ///
    /// Special care must be taken when sharing pages with other address spaces,
    /// e.g. by setting the `GLOBAL` flag. For example, a global mapping must be
    /// the same in all address spaces, otherwise undefined behavior can occur
    /// because of TLB races. It's worth noting that all the above requirements
    /// also apply to shared mappings, including the aliasing requirements.
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<S>,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<S>, MapToError<S>>
    where
        Self: Sized,
        A: FrameAllocator<Size4KiB> + ?Sized;

    /// Removes a mapping from the page table and returns the frame that used to be mapped.
    ///
    /// Note that no page tables or pages are deallocated.
    fn unmap(&mut self, page: Page<S>) -> Result<(PhysFrame<S>, MapperFlush<S>), UnmapError>;

    /// Updates the flags of an existing mapping.
    ///
    /// ## Safety
    ///
    /// This method is unsafe because changing the flags of a mapping
    /// might result in undefined behavior. For example, setting the
    /// `GLOBAL` and `MUTABLE` flags for a page might result in the corruption
    /// of values stored in that page from processes running in other address
    /// spaces.
    unsafe fn update_flags(
        &mut self,
        page: Page<S>,
        flags: PageTableFlags,
    ) -> Result<MapperFlush<S>, FlagUpdateError>;

    /// Return the frame that the specified page is mapped to.
    ///
    /// This function assumes that the page is mapped to a frame of size `S` and returns an
    /// error otherwise.
    fn translate_page(&self, page: Page<S>) -> Result<PhysFrame<S>, TranslateError>;

    /// Maps the given frame to the virtual page with the same address.
    ///
    /// ## Safety
    ///
    /// This is a convencience function that invokes [`Mapper::map_to`] internally, so
    /// all safety requirements of it also apply for this function.
    #[inline]
    unsafe fn identity_map<A>(
        &mut self,
        frame: PhysFrame<S>,
        flags: PageTableFlags,
        frame_allocator: &mut A,
    ) -> Result<MapperFlush<S>, MapToError<S>>
    where
        Self: Sized,
        A: FrameAllocator<Size4KiB> + ?Sized,
        S: PageSize,
        Self: Mapper<S>,
    {
        let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));
        self.map_to(page, frame, flags, frame_allocator)
    }
}

/// This type represents a page whose mapping has changed in the page table.
///
/// The old mapping might be still cached in the translation lookaside buffer (TLB), so it needs
/// to be flushed from the TLB before it's accessed. This type is returned from function that
/// change the mapping of a page to ensure that the TLB flush is not forgotten.
#[derive(Debug)]
#[must_use = "Page Table changes must be flushed or ignored."]
pub struct MapperFlush<S: PageSize>(Page<S>);

impl<S: PageSize> MapperFlush<S> {
    /// Create a new flush promise
    #[inline]
    fn new(page: Page<S>) -> Self {
        MapperFlush(page)
    }

    /// Flush the page from the TLB to ensure that the newest mapping is used.
    #[inline]
    pub fn flush(self) {
        let raw = self.0.start_address().as_u64();

        unsafe {
            asm!("invlpg [{}]", in(reg) raw, options(nostack));
        }
    }
}

/// This error is returned from `map_to` and similar methods.
#[derive(Debug)]
pub enum MapToError<S: PageSize> {
    /// An additional frame was needed for the mapping process, but the frame allocator
    /// returned `None`.
    FrameAllocationFailed,
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of an already mapped huge page.
    ParentEntryHugePage,
    /// The given page is already mapped to a physical frame.
    PageAlreadyMapped(PhysFrame<S>),
}

/// An error indicating that an `unmap` call failed.
#[derive(Debug)]
pub enum UnmapError {
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of a huge page and can't be freed individually.
    ParentEntryHugePage,
    /// The given page is not mapped to a physical frame.
    PageNotMapped,
    /// The page table entry for the given page points to an invalid physical address.
    InvalidFrameAddress(PhysAddr),
}

/// An error indicating that an `update_flags` call failed.
#[derive(Debug)]
pub enum FlagUpdateError {
    /// The given page is not mapped to a physical frame.
    PageNotMapped,
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of a huge page and can't be freed individually.
    ParentEntryHugePage,
}

/// An error indicating that an `translate` call failed.
#[derive(Debug)]
pub enum TranslateError {
    /// The given page is not mapped to a physical frame.
    PageNotMapped,
    /// An upper level page table entry has the `HUGE_PAGE` flag set, which means that the
    /// given page is part of a huge page and can't be freed individually.
    ParentEntryHugePage,
    /// The page table entry for the given page points to an invalid physical address.
    InvalidFrameAddress(PhysAddr),
}

/// A trait for types that can deallocate a frame of memory.
pub trait FrameDeallocator<S: PageSize> {
    /// Deallocate the given unused frame.
    ///
    /// ## Safety
    ///
    /// The caller must ensure that the passed frame is unused.
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<S>);
}

/// A Mapper implementation that relies on a PhysAddr to VirtAddr conversion function.
///
/// This type requires that the all physical page table frames are mapped to some virtual
/// address. Normally, this is done by mapping the complete physical address space into
/// the virtual address space at some offset. Other mappings between physical and virtual
/// memory are possible too, as long as they can be calculated as an `PhysAddr` to
/// `VirtAddr` closure.
#[derive(Debug)]
pub struct MappedPageTable<'a, P: PageTableFrameMapping> {
    page_table_walker: PageTableWalker<P>,
    level_5_paging_enabled: bool,
    page_table: &'a mut PageTable,
}

impl<'a, P: PageTableFrameMapping> MappedPageTable<'a, P> {
    /// Creates a new `MappedPageTable` that uses the passed closure for converting virtual
    /// to physical addresses.
    ///
    /// ## Safety
    ///
    /// This function is unsafe because the caller must guarantee that the passed `page_table_frame_mapping`
    /// closure is correct. Also, the passed `page_table` must point to the level 4 page table
    /// of a valid page table hierarchy. Otherwise this function might break memory safety, e.g.
    /// by writing to an illegal memory location.
    #[inline]
    pub unsafe fn new(page_table: &'a mut PageTable, page_table_frame_mapping: P) -> Self {
        Self {
            page_table,
            level_5_paging_enabled: super::level_5_paging_enabled(),
            page_table_walker: PageTableWalker::new(page_table_frame_mapping),
        }
    }

    fn map_to_2mib<A>(
        &mut self,
        page: Page<Size2MiB>,
        frame: PhysFrame<Size2MiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size2MiB>, MapToError<Size2MiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let mut is_alloc_4 = false;

        let p4;

        if self.level_5_paging_enabled {
            let p5 = &mut self.page_table;
            let (alloc, yes) = self.page_table_walker.create_next_table(
                &mut p5[page.p5_index()],
                parent_table_flags,
                allocator,
            )?;

            p4 = yes;
            is_alloc_4 = alloc;
        } else {
            p4 = &mut self.page_table;
        }

        let (is_alloc_3, p3) = self.page_table_walker.create_next_table(
            &mut p4[page.p4_index()],
            parent_table_flags,
            allocator,
        )?;

        let (is_alloc_2, p2) = self.page_table_walker.create_next_table(
            &mut p3[page.p3_index()],
            parent_table_flags,
            allocator,
        )?;

        if !p2[page.p2_index()].is_unused() {
            return Err(MapToError::PageAlreadyMapped(frame));
        }

        p2[page.p2_index()].set_addr(frame.start_address(), flags | PageTableFlags::HUGE_PAGE);

        if is_alloc_2 {
            p3[page.p3_index()].inc_entry_count();
        }

        if is_alloc_3 {
            p4[page.p4_index()].inc_entry_count();
        }

        if is_alloc_4 {
            let p5 = &mut self.page_table;
            p5[page.p5_index()].inc_entry_count();
        }

        Ok(MapperFlush::new(page))
    }

    fn map_to_4kib<A>(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let p4;

        let mut is_alloc_4 = false;

        if self.level_5_paging_enabled {
            let p5 = &mut self.page_table;
            let (alloc, yes) = self.page_table_walker.create_next_table(
                &mut p5[page.p5_index()],
                parent_table_flags,
                allocator,
            )?;

            p4 = yes;
            is_alloc_4 = alloc;
        } else {
            p4 = &mut self.page_table;
        }

        let (is_alloc_3, p3) = self.page_table_walker.create_next_table(
            &mut p4[page.p4_index()],
            parent_table_flags,
            allocator,
        )?;

        let (is_alloc_2, p2) = self.page_table_walker.create_next_table(
            &mut p3[page.p3_index()],
            parent_table_flags,
            allocator,
        )?;

        let (is_alloc_1, p1) = self.page_table_walker.create_next_table(
            &mut p2[page.p2_index()],
            parent_table_flags,
            allocator,
        )?;

        if !p1[page.p1_index()].is_unused() {
            return Err(MapToError::PageAlreadyMapped(frame));
        }

        p1[page.p1_index()].set_frame(frame, flags);

        if is_alloc_1 {
            p2[page.p2_index()].inc_entry_count();
        }

        if is_alloc_2 {
            p3[page.p3_index()].inc_entry_count();
        }

        if is_alloc_3 {
            p4[page.p4_index()].inc_entry_count();
        }

        if is_alloc_4 {
            let p5 = &mut self.page_table;
            p5[page.p5_index()].inc_entry_count();
        }

        Ok(MapperFlush::new(page))
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size2MiB> for MappedPageTable<'a, P> {
    #[inline]
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<Size2MiB>,
        frame: PhysFrame<Size2MiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size2MiB>, MapToError<Size2MiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        self.map_to_2mib(page, frame, flags, parent_table_flags, allocator)
    }

    fn unmap(
        &mut self,
        page: Page<Size2MiB>,
    ) -> Result<(PhysFrame<Size2MiB>, MapperFlush<Size2MiB>), UnmapError> {
        let p4;

        if self.level_5_paging_enabled {
            let p5 = &mut self.page_table;

            p4 = self
                .page_table_walker
                .next_table_mut(&mut p5[page.p5_index()])?;
        } else {
            p4 = &mut self.page_table;
        }

        let p3 = self
            .page_table_walker
            .next_table_mut(&mut p4[page.p4_index()])?;
        let p2 = self
            .page_table_walker
            .next_table_mut(&mut p3[page.p3_index()])?;

        let p2_entry = &mut p2[page.p2_index()];
        let flags = p2_entry.flags();

        if !flags.contains(PageTableFlags::PRESENT) {
            return Err(UnmapError::PageNotMapped);
        }
        if !flags.contains(PageTableFlags::HUGE_PAGE) {
            return Err(UnmapError::ParentEntryHugePage);
        }

        let frame = PhysFrame::from_start_address(p2_entry.addr())
            .map_err(|AddressNotAligned| UnmapError::InvalidFrameAddress(p2_entry.addr()))?;

        p2_entry.unref_vm_frame();
        p2_entry.set_unused();

        let p3_entry = &mut p3[page.p3_index()];
        p3_entry.dec_entry_count();

        if p3_entry.get_entry_count() == 0 {
            p3_entry.unref_vm_frame();
            p3_entry.set_unused();
        }

        Ok((frame, MapperFlush::new(page)))
    }

    unsafe fn update_flags(
        &mut self,
        page: Page<Size2MiB>,
        flags: PageTableFlags,
    ) -> Result<MapperFlush<Size2MiB>, FlagUpdateError> {
        let p4;

        if self.level_5_paging_enabled {
            let p5 = &mut self.page_table;

            p4 = self
                .page_table_walker
                .next_table_mut(&mut p5[page.p5_index()])?;
        } else {
            p4 = &mut self.page_table;
        }

        let p3 = self
            .page_table_walker
            .next_table_mut(&mut p4[page.p4_index()])?;
        let p2 = self
            .page_table_walker
            .next_table_mut(&mut p3[page.p3_index()])?;

        if p2[page.p2_index()].is_unused() {
            return Err(FlagUpdateError::PageNotMapped);
        }

        p2[page.p2_index()].set_flags(flags | PageTableFlags::HUGE_PAGE);

        Ok(MapperFlush::new(page))
    }

    fn translate_page(&self, page: Page<Size2MiB>) -> Result<PhysFrame<Size2MiB>, TranslateError> {
        let p4;

        if self.level_5_paging_enabled {
            let p5 = &self.page_table;

            p4 = self.page_table_walker.next_table(&p5[page.p5_index()])?;
        } else {
            p4 = &self.page_table;
        }

        let p3 = self.page_table_walker.next_table(&p4[page.p4_index()])?;
        let p2 = self.page_table_walker.next_table(&p3[page.p3_index()])?;

        let p2_entry = &p2[page.p2_index()];

        if p2_entry.is_unused() {
            return Err(TranslateError::PageNotMapped);
        }

        PhysFrame::from_start_address(p2_entry.addr())
            .map_err(|AddressNotAligned| TranslateError::InvalidFrameAddress(p2_entry.addr()))
    }
}

impl<'a, P: PageTableFrameMapping> Mapper<Size4KiB> for MappedPageTable<'a, P> {
    #[inline]
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        self.map_to_4kib(page, frame, flags, parent_table_flags, allocator)
    }

    fn unmap(
        &mut self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysFrame<Size4KiB>, MapperFlush<Size4KiB>), UnmapError> {
        let p4;

        if self.level_5_paging_enabled {
            let p5 = &mut self.page_table;

            p4 = self
                .page_table_walker
                .next_table_mut(&mut p5[page.p5_index()])?;
        } else {
            p4 = &mut self.page_table;
        }

        let p3 = self
            .page_table_walker
            .next_table_mut(&mut p4[page.p4_index()])?;
        let p2 = self
            .page_table_walker
            .next_table_mut(&mut p3[page.p3_index()])?;
        let p1 = self
            .page_table_walker
            .next_table_mut(&mut p2[page.p2_index()])?;

        let p1_entry = &mut p1[page.p1_index()];

        let frame = p1_entry.frame().map_err(|err| match err {
            FrameError::FrameNotPresent => UnmapError::PageNotMapped,
            FrameError::HugeFrame => UnmapError::ParentEntryHugePage,
        })?;

        p1_entry.unref_vm_frame();
        p1_entry.set_unused();

        let p2_entry = &mut p2[page.p2_index()];
        p2_entry.dec_entry_count();

        if p2_entry.get_entry_count() == 0 {
            p2_entry.unref_vm_frame();
            p2_entry.set_unused();
        }

        Ok((frame, MapperFlush::new(page)))
    }

    unsafe fn update_flags(
        &mut self,
        page: Page<Size4KiB>,
        flags: PageTableFlags,
    ) -> Result<MapperFlush<Size4KiB>, FlagUpdateError> {
        let p4;

        if self.level_5_paging_enabled {
            let p5 = &mut self.page_table;

            p4 = self
                .page_table_walker
                .next_table_mut(&mut p5[page.p5_index()])?;
        } else {
            p4 = &mut self.page_table;
        }

        let p3 = self
            .page_table_walker
            .next_table_mut(&mut p4[page.p4_index()])?;
        let p2 = self
            .page_table_walker
            .next_table_mut(&mut p3[page.p3_index()])?;
        let p1 = self
            .page_table_walker
            .next_table_mut(&mut p2[page.p2_index()])?;

        if p1[page.p1_index()].is_unused() {
            return Err(FlagUpdateError::PageNotMapped);
        }

        p1[page.p1_index()].set_flags(flags);

        Ok(MapperFlush::new(page))
    }

    fn translate_page(&self, page: Page<Size4KiB>) -> Result<PhysFrame<Size4KiB>, TranslateError> {
        let p4;

        if self.level_5_paging_enabled {
            let p5 = &self.page_table;

            p4 = self.page_table_walker.next_table(&p5[page.p5_index()])?;
        } else {
            p4 = &self.page_table;
        }

        let p3 = self.page_table_walker.next_table(&p4[page.p4_index()])?;
        let p2 = self.page_table_walker.next_table(&p3[page.p3_index()])?;
        let p1 = self.page_table_walker.next_table(&p2[page.p2_index()])?;

        let p1_entry = &p1[page.p1_index()];

        if p1_entry.is_unused() {
            return Err(TranslateError::PageNotMapped);
        }

        PhysFrame::from_start_address(p1_entry.addr())
            .map_err(|AddressNotAligned| TranslateError::InvalidFrameAddress(p1_entry.addr()))
    }
}

impl<'a, P: PageTableFrameMapping> Translate for MappedPageTable<'a, P> {
    #[allow(clippy::inconsistent_digit_grouping)]
    fn translate(&self, addr: VirtAddr) -> TranslateResult {
        let p4;

        if self.level_5_paging_enabled {
            let p5 = &self.page_table;

            p4 = match self.page_table_walker.next_table(&p5[addr.p5_index()]) {
                Ok(page_table) => page_table,
                Err(PageTableWalkError::NotMapped) => return TranslateResult::NotMapped,
                Err(PageTableWalkError::MappedToHugePage) => {
                    panic!("level 4 entry has huge page bit set")
                }
            };
        } else {
            p4 = &self.page_table;
        }

        let p3 = match self.page_table_walker.next_table(&p4[addr.p4_index()]) {
            Ok(page_table) => page_table,
            Err(PageTableWalkError::NotMapped) => return TranslateResult::NotMapped,
            Err(PageTableWalkError::MappedToHugePage) => {
                panic!("level 4 entry has huge page bit set")
            }
        };
        let p2 = match self.page_table_walker.next_table(&p3[addr.p3_index()]) {
            Ok(page_table) => page_table,
            Err(PageTableWalkError::NotMapped) => return TranslateResult::NotMapped,
            Err(PageTableWalkError::MappedToHugePage) => {
                let entry = &p3[addr.p3_index()];
                let frame = PhysFrame::containing_address(entry.addr());
                let offset = addr.as_u64() & 0o_777_777_7777;
                let flags = entry.flags();
                return TranslateResult::Mapped {
                    frame: MappedFrame::Size1GiB(frame),
                    offset,
                    flags,
                };
            }
        };
        let p1 = match self.page_table_walker.next_table(&p2[addr.p2_index()]) {
            Ok(page_table) => page_table,
            Err(PageTableWalkError::NotMapped) => return TranslateResult::NotMapped,
            Err(PageTableWalkError::MappedToHugePage) => {
                let entry = &p2[addr.p2_index()];
                let frame = PhysFrame::containing_address(entry.addr());
                let offset = addr.as_u64() & 0o_777_7777;
                let flags = entry.flags();
                return TranslateResult::Mapped {
                    frame: MappedFrame::Size2MiB(frame),
                    offset,
                    flags,
                };
            }
        };

        let p1_entry = &p1[addr.p1_index()];

        if p1_entry.is_unused() {
            return TranslateResult::NotMapped;
        }

        let frame = match PhysFrame::from_start_address(p1_entry.addr()) {
            Ok(frame) => frame,
            Err(AddressNotAligned) => return TranslateResult::InvalidFrameAddress(p1_entry.addr()),
        };
        let offset = u64::from(addr.page_offset());
        let flags = p1_entry.flags();
        TranslateResult::Mapped {
            frame: MappedFrame::Size4KiB(frame),
            offset,
            flags,
        }
    }
}

#[derive(Debug)]
struct PageTableWalker<P: PageTableFrameMapping> {
    page_table_frame_mapping: P,
}

impl<P: PageTableFrameMapping> PageTableWalker<P> {
    #[inline]
    pub unsafe fn new(page_table_frame_mapping: P) -> Self {
        Self {
            page_table_frame_mapping,
        }
    }

    /// Internal helper function to get a reference to the page table of the next level.
    ///
    /// Returns `PageTableWalkError::NotMapped` if the entry is unused. Returns
    /// `PageTableWalkError::MappedToHugePage` if the `HUGE_PAGE` flag is set
    /// in the passed entry.
    #[inline]
    fn next_table<'b>(
        &self,
        entry: &'b PageTableEntry,
    ) -> Result<&'b PageTable, PageTableWalkError> {
        let page_table_ptr = self
            .page_table_frame_mapping
            .frame_to_pointer(entry.frame()?);
        let page_table: &PageTable = unsafe { &*page_table_ptr };

        Ok(page_table)
    }

    /// Internal helper function to get a mutable reference to the page table of the next level.
    ///
    /// Returns `PageTableWalkError::NotMapped` if the entry is unused. Returns
    /// `PageTableWalkError::MappedToHugePage` if the `HUGE_PAGE` flag is set
    /// in the passed entry.
    #[inline]
    fn next_table_mut<'b>(
        &self,
        entry: &'b mut PageTableEntry,
    ) -> Result<&'b mut PageTable, PageTableWalkError> {
        let page_table_ptr = self
            .page_table_frame_mapping
            .frame_to_pointer(entry.frame()?);
        let page_table: &mut PageTable = unsafe { &mut *page_table_ptr };

        Ok(page_table)
    }

    /// Internal helper function to create the page table of the next level if needed.
    ///
    /// If the passed entry is unused, a new frame is allocated from the given allocator, zeroed,
    /// and the entry is updated to that address. If the passed entry is already mapped, the next
    /// table is returned directly.
    ///
    /// Returns `MapToError::FrameAllocationFailed` if the entry is unused and the allocator
    /// returned `None`. Returns `MapToError::ParentEntryHugePage` if the `HUGE_PAGE` flag is set
    /// in the passed entry.
    fn create_next_table<'b, A>(
        &self,
        entry: &'b mut PageTableEntry,
        insert_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<(bool, &'b mut PageTable), PageTableCreateError>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        let created;

        if entry.is_unused() {
            if let Some(frame) = allocator.allocate_frame() {
                entry.set_frame(frame, insert_flags);
                created = true;
            } else {
                return Err(PageTableCreateError::FrameAllocationFailed);
            }
        } else {
            if !insert_flags.is_empty() && !entry.flags().contains(insert_flags) {
                entry.set_flags(entry.flags() | insert_flags);
            }
            created = false;
        }

        let page_table = match self.next_table_mut(entry) {
            Err(PageTableWalkError::MappedToHugePage) => {
                return Err(PageTableCreateError::MappedToHugePage);
            }
            Err(PageTableWalkError::NotMapped) => {
                unreachable!("entry should be mapped at this point")
            }
            Ok(page_table) => page_table,
        };

        if created {
            page_table.zero();
        }

        Ok((created, page_table))
    }
}

#[derive(Debug)]
enum PageTableWalkError {
    NotMapped,
    MappedToHugePage,
}

#[derive(Debug)]
enum PageTableCreateError {
    MappedToHugePage,
    FrameAllocationFailed,
}

impl From<PageTableCreateError> for MapToError<Size4KiB> {
    #[inline]
    fn from(err: PageTableCreateError) -> Self {
        match err {
            PageTableCreateError::MappedToHugePage => MapToError::ParentEntryHugePage,
            PageTableCreateError::FrameAllocationFailed => MapToError::FrameAllocationFailed,
        }
    }
}

impl From<PageTableCreateError> for MapToError<Size2MiB> {
    #[inline]
    fn from(err: PageTableCreateError) -> Self {
        match err {
            PageTableCreateError::MappedToHugePage => MapToError::ParentEntryHugePage,
            PageTableCreateError::FrameAllocationFailed => MapToError::FrameAllocationFailed,
        }
    }
}

impl From<PageTableCreateError> for MapToError<Size1GiB> {
    #[inline]
    fn from(err: PageTableCreateError) -> Self {
        match err {
            PageTableCreateError::MappedToHugePage => MapToError::ParentEntryHugePage,
            PageTableCreateError::FrameAllocationFailed => MapToError::FrameAllocationFailed,
        }
    }
}

impl From<FrameError> for PageTableWalkError {
    #[inline]
    fn from(err: FrameError) -> Self {
        match err {
            FrameError::HugeFrame => PageTableWalkError::MappedToHugePage,
            FrameError::FrameNotPresent => PageTableWalkError::NotMapped,
        }
    }
}

impl From<PageTableWalkError> for UnmapError {
    #[inline]
    fn from(err: PageTableWalkError) -> Self {
        match err {
            PageTableWalkError::MappedToHugePage => UnmapError::ParentEntryHugePage,
            PageTableWalkError::NotMapped => UnmapError::PageNotMapped,
        }
    }
}

impl From<PageTableWalkError> for FlagUpdateError {
    #[inline]
    fn from(err: PageTableWalkError) -> Self {
        match err {
            PageTableWalkError::MappedToHugePage => FlagUpdateError::ParentEntryHugePage,
            PageTableWalkError::NotMapped => FlagUpdateError::PageNotMapped,
        }
    }
}

impl From<PageTableWalkError> for TranslateError {
    #[inline]
    fn from(err: PageTableWalkError) -> Self {
        match err {
            PageTableWalkError::MappedToHugePage => TranslateError::ParentEntryHugePage,
            PageTableWalkError::NotMapped => TranslateError::PageNotMapped,
        }
    }
}

/// Provides a virtual address mapping for physical page table frames.
///
/// This only works if the physical address space is somehow mapped to the virtual
/// address space, e.g. at an offset.
///
/// ## Safety
///
/// This trait is unsafe to implement because the implementer must ensure that
/// `frame_to_pointer` returns a valid page table pointer for any given physical frame.
pub unsafe trait PageTableFrameMapping {
    /// Translate the given physical frame to a virtual page table pointer.
    fn frame_to_pointer(&self, frame: PhysFrame) -> *mut PageTable;
}

/// A Mapper implementation that requires that the complete physically memory is mapped at some
/// offset in the virtual address space.
#[derive(Debug)]
pub struct OffsetPageTable<'a> {
    inner: MappedPageTable<'a, PhysOffset>,
}

impl<'a> OffsetPageTable<'a> {
    /// Creates a new `OffsetPageTable` that uses the given offset for converting virtual
    /// to physical addresses.
    ///
    /// The complete physical memory must be mapped in the virtual address space starting at
    /// address `phys_offset`. This means that for example physical address `0x5000` can be
    /// accessed through virtual address `phys_offset + 0x5000`. This mapping is required because
    /// the mapper needs to access page tables, which are not mapped into the virtual address
    /// space by default.
    ///
    /// ## Safety
    ///
    /// This function is unsafe because the caller must guarantee that the passed `phys_offset`
    /// is correct. Also, the passed `page_table` must point to the level 4 page table
    /// of a valid page table hierarchy. Otherwise this function might break memory safety, e.g.
    /// by writing to an illegal memory location.
    #[inline]
    pub unsafe fn new(page_table: &'a mut PageTable, phys_offset: VirtAddr) -> Self {
        let phys_offset = PhysOffset {
            offset: phys_offset,
        };
        Self {
            inner: MappedPageTable::new(page_table, phys_offset),
        }
    }
}

#[derive(Debug)]
struct PhysOffset {
    offset: VirtAddr,
}

unsafe impl PageTableFrameMapping for PhysOffset {
    fn frame_to_pointer(&self, frame: PhysFrame) -> *mut PageTable {
        let virt = self.offset + frame.start_address().as_u64();
        virt.as_mut_ptr()
    }
}

impl<'a> Mapper<Size2MiB> for OffsetPageTable<'a> {
    #[inline]
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<Size2MiB>,
        frame: PhysFrame<Size2MiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size2MiB>, MapToError<Size2MiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        self.inner
            .map_to_with_table_flags(page, frame, flags, parent_table_flags, allocator)
    }

    #[inline]
    fn unmap(
        &mut self,
        page: Page<Size2MiB>,
    ) -> Result<(PhysFrame<Size2MiB>, MapperFlush<Size2MiB>), UnmapError> {
        self.inner.unmap(page)
    }

    #[inline]
    unsafe fn update_flags(
        &mut self,
        page: Page<Size2MiB>,
        flags: PageTableFlags,
    ) -> Result<MapperFlush<Size2MiB>, FlagUpdateError> {
        self.inner.update_flags(page, flags)
    }

    #[inline]
    fn translate_page(&self, page: Page<Size2MiB>) -> Result<PhysFrame<Size2MiB>, TranslateError> {
        self.inner.translate_page(page)
    }
}

impl<'a> Mapper<Size4KiB> for OffsetPageTable<'a> {
    #[inline]
    unsafe fn map_to_with_table_flags<A>(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
        parent_table_flags: PageTableFlags,
        allocator: &mut A,
    ) -> Result<MapperFlush<Size4KiB>, MapToError<Size4KiB>>
    where
        A: FrameAllocator<Size4KiB> + ?Sized,
    {
        self.inner
            .map_to_with_table_flags(page, frame, flags, parent_table_flags, allocator)
    }

    #[inline]
    fn unmap(
        &mut self,
        page: Page<Size4KiB>,
    ) -> Result<(PhysFrame<Size4KiB>, MapperFlush<Size4KiB>), UnmapError> {
        self.inner.unmap(page)
    }

    #[inline]
    unsafe fn update_flags(
        &mut self,
        page: Page<Size4KiB>,
        flags: PageTableFlags,
    ) -> Result<MapperFlush<Size4KiB>, FlagUpdateError> {
        self.inner.update_flags(page, flags)
    }

    #[inline]
    fn translate_page(&self, page: Page<Size4KiB>) -> Result<PhysFrame<Size4KiB>, TranslateError> {
        self.inner.translate_page(page)
    }
}

impl<'a> Translate for OffsetPageTable<'a> {
    #[inline]
    fn translate(&self, addr: VirtAddr) -> TranslateResult {
        self.inner.translate(addr)
    }
}

impl<'a> OffsetPageTable<'a> {
    pub fn unmap_range(&mut self, range: Range<VirtAddr>, step: u64) -> Result<(), UnmapError> {
        for addr in range.step_by(step as usize) {
            let page: Page = Page::containing_address(addr);
            self.inner.unmap(page)?.1.flush();
        }

        Ok(())
    }

    pub fn fork(&mut self) -> Result<AddressSpace, MapToError<Size4KiB>> {
        let mut address_space = AddressSpace::new()?; // Allocate the new address space

        let offset_table = address_space.offset_page_table();
        let make_next_level = |table: &mut PageTable,
                               i: usize|
         -> Result<(bool, &mut PageTable), MapToError<Size4KiB>> {
            let entry = &mut table[i];
            let created;

            if !entry.flags().contains(PageTableFlags::PRESENT) {
                let frame = unsafe { FRAME_ALLOCATOR.allocate_frame() }
                    .ok_or(MapToError::FrameAllocationFailed)?;

                entry.set_frame(
                    frame,
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::USER_ACCESSIBLE,
                );

                created = true;
            } else {
                entry.set_flags(
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::USER_ACCESSIBLE,
                );

                created = false;
            }

            let page_table_ptr = unsafe {
                let addr =
                    crate::PHYSICAL_MEMORY_OFFSET + entry.frame().unwrap().start_address().as_u64();

                addr.as_mut_ptr::<PageTable>()
            };

            let page_table: &mut PageTable = unsafe { &mut *page_table_ptr };
            if created {
                page_table.zero();
            }

            Ok((created, page_table))
        };

        let last_level_fork = |entry: &mut PageTableEntry, n1: &mut PageTable, i: usize| {
            let mut flags = entry.flags();

            flags.remove(PageTableFlags::WRITABLE | PageTableFlags::ACCESSED);

            entry.set_flags(flags);
            n1[i].set_frame(entry.frame().unwrap(), flags);
        };

        // We loop through each of the page table entries in the page table which are user
        // accessable and we remove the writeable flag from the entry if present. This will
        // make the page table entry copy on the first write. Then we clone the page table entry
        // and place it in the new page table.
        if self.inner.level_5_paging_enabled {
            self.inner.page_table.for_entries_mut(
                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                |i, _, table| {
                    let (_, n4) = make_next_level(offset_table.inner.page_table, i)?;
                    let mut count_4 = 0;

                    table.for_entries_mut(
                        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                        |j, _, table| {
                            let (w3, n3) = make_next_level(n4, j)?;
                            let mut count_3 = 0;

                            if w3 {
                                count_4 += 1;
                            }

                            table.for_entries_mut(
                                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                                |k, _, table| {
                                    let (w2, n2) = make_next_level(n3, k)?;
                                    let mut count_2 = 0;

                                    if w2 {
                                        count_3 += 1;
                                    }

                                    table.for_entries_mut(
                                        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                                        |l, _, table| {
                                            let (w1, n1) = make_next_level(n2, l)?;
                                            let mut count_1 = 0;

                                            if w1 {
                                                count_2 += 1;
                                            }

                                            table.for_entries_mut(
                                                PageTableFlags::PRESENT
                                                    | PageTableFlags::USER_ACCESSIBLE,
                                                |i, entry, _| {
                                                    last_level_fork(entry, n1, i);

                                                    count_1 += 1;
                                                    Ok(())
                                                },
                                            )?;

                                            n2[l].set_entry_count(count_1);
                                            Ok(())
                                        },
                                    )?;

                                    n3[k].set_entry_count(count_2);
                                    Ok(())
                                },
                            )?;

                            n4[j].set_entry_count(count_3);
                            Ok(())
                        },
                    )?;

                    offset_table.inner.page_table[i].set_entry_count(count_4);
                    Ok(())
                },
            )?;
        } else {
            self.inner.page_table.for_entries_mut(
                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                |i, _, table| {
                    let (_, n3) = make_next_level(offset_table.inner.page_table, i)?;
                    let mut count_3 = 0;

                    table.for_entries_mut(
                        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                        |k, _, table| {
                            let (w2, n2) = make_next_level(n3, k)?;
                            let mut count_2 = 0;

                            if w2 {
                                count_3 += 1;
                            }

                            table.for_entries_mut(
                                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                                |l, _, table| {
                                    let (w1, n1) = make_next_level(n2, l)?;
                                    let mut count_1 = 0;

                                    if w1 {
                                        count_2 += 1;
                                    }

                                    table.for_entries_mut(
                                        PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                                        |i, entry, _| {
                                            last_level_fork(entry, n1, i);

                                            count_1 += 1;
                                            Ok(())
                                        },
                                    )?;

                                    n2[l].set_entry_count(count_1);
                                    Ok(())
                                },
                            )?;

                            n3[k].set_entry_count(count_2);
                            Ok(())
                        },
                    )?;

                    offset_table.inner.page_table[i].set_entry_count(count_3);
                    Ok(())
                },
            )?;
        }

        Ok(address_space)
    }
}
