use core::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use super::{address::VirtualAddress, NotGiantPageSize, PageSize, Size4KiB};

const PAGE_TABLE_ENTRY_COUNT: usize = 512;

bitflags::bitflags! {
    /// Page table flags for a page table entry.
    pub struct PageTableFlags: u64 {
        const PRESENT =         1;
        const WRITABLE =        1 << 1;
        const USER_ACCESSIBLE = 1 << 2;
        const WRITE_THROUGH =   1 << 3;
        const NO_CACHE =        1 << 4;
        const ACCESSED =        1 << 5;
        const DIRTY =           1 << 6;
        const HUGE_PAGE =       1 << 7;
        const GLOBAL =          1 << 8;
        const BIT_9 =           1 << 9;
        const BIT_10 =          1 << 10;
        const BIT_11 =          1 << 11;
        const BIT_52 =          1 << 52;
        const BIT_53 =          1 << 53;
        const BIT_54 =          1 << 54;
        const BIT_55 =          1 << 55;
        const BIT_56 =          1 << 56;
        const BIT_57 =          1 << 57;
        const BIT_58 =          1 << 58;
        const BIT_59 =          1 << 59;
        const BIT_60 =          1 << 60;
        const BIT_61 =          1 << 61;
        const BIT_62 =          1 << 62;
        const NO_EXECUTE =      1 << 63;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {}

#[repr(align(4096))]
#[repr(C)]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRY_COUNT],
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl Index<u64> for PageTable {
    type Output = PageTableEntry;

    #[inline]
    fn index(&self, index: u64) -> &Self::Output {
        &self.entries[index as usize]
    }
}

impl IndexMut<u64> for PageTable {
    #[inline]
    fn index_mut(&mut self, index: u64) -> &mut Self::Output {
        &mut self.entries[index as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Page<S: PageSize = Size4KiB> {
    start_address: VirtualAddress,
    size: PhantomData<S>,
}

impl<S: PageSize> Page<S> {
    pub const SIZE: u64 = S::SIZE;

    #[inline]
    pub fn containing_address(address: VirtualAddress) -> Self {
        Self {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    #[inline(always)]
    pub fn start_address(self) -> VirtualAddress {
        self.start_address
    }

    #[inline(always)]
    pub fn p4_index(self) -> u16 {
        self.start_address().p4_index()
    }

    #[inline(always)]
    pub fn p3_index(self) -> u16 {
        self.start_address().p3_index()
    }
}

impl<S: NotGiantPageSize> Page<S> {
    #[inline(always)]
    pub fn p2_index(self) -> u16 {
        self.start_address().p2_index()
    }
}

impl Page<Size4KiB> {
    #[inline(always)]
    pub fn p1_index(self) -> u16 {
        self.start_address().p1_index()
    }
}
