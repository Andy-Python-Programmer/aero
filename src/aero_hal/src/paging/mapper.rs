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

    pub fn map_to_1gib<A>(
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
        todo!()
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

#[derive(Debug)]
struct PageTableWalker(VirtualAddress);

impl PageTableWalker {
    #[inline(always)]
    fn new(physical_memory_offset: VirtualAddress) -> Self {
        Self(physical_memory_offset)
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
