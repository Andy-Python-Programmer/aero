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

use core::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use super::{
    address::{PhysicalAddress, VirtualAddress},
    frame::Frame,
    NotGiantPageSize, PageSize, Size4KiB,
};

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

impl PageTableEntry {
    #[inline(always)]
    pub fn address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.0 & 0x000f_ffff_ffff_f000)
    }

    #[inline(always)]
    pub const fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.0)
    }

    #[inline]
    pub fn frame(&self) -> Frame {
        assert!(self.flags().contains(PageTableFlags::PRESENT));
        assert!(!self.flags().contains(PageTableFlags::HUGE_PAGE));

        Frame::containing_address(self.address())
    }

    #[inline(always)]
    pub const fn is_unused(&self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    #[inline]
    pub fn set_address(&mut self, address: PhysicalAddress, flags: PageTableFlags) {
        assert!(address.is_aligned(Size4KiB::SIZE));

        self.0 = (address.as_u64()) | flags.bits();
    }

    /// Map the entry to the specified physical frame with the specified flags.
    #[inline]
    pub fn set_frame(&mut self, frame: Frame, flags: PageTableFlags) {
        assert!(!flags.contains(PageTableFlags::HUGE_PAGE));

        self.set_address(frame.start_address(), flags)
    }
}

#[repr(align(4096))]
#[repr(C)]
pub struct PageTable {
    entries: [PageTableEntry; PAGE_TABLE_ENTRY_COUNT],
}

impl PageTable {
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }
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

impl Index<u16> for PageTable {
    type Output = PageTableEntry;

    #[inline]
    fn index(&self, index: u16) -> &Self::Output {
        &self.entries[index as usize]
    }
}

impl IndexMut<u16> for PageTable {
    #[inline]
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
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
