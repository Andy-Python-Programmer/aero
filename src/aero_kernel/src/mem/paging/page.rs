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

//! Abstractions for default-sized and huge virtual memory pages.

use super::addr::{PhysAddr, VirtAddr};
use super::page_table::PageTableIndex;

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

/// Trait for abstracting over the three possible page sizes on x86_64, 4KiB, 2MiB, 1GiB.
pub trait PageSize: Copy + Eq + PartialOrd + Ord {
    /// The page size in bytes.
    const SIZE: u64;

    /// A string representation of the page size for debug output.
    const SIZE_AS_DEBUG_STR: &'static str;
}

/// This trait is implemented for 4KiB and 2MiB pages, but not for 1GiB pages.
pub trait NotGiantPageSize: PageSize {}

/// A standard 4KiB page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size4KiB {}

/// A “huge” 2MiB page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size2MiB {}

/// A “giant” 1GiB page.
///
/// (Only available on newer x86_64 CPUs.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size1GiB {}

impl PageSize for Size4KiB {
    const SIZE: u64 = 4096;
    const SIZE_AS_DEBUG_STR: &'static str = "4KiB";
}

impl NotGiantPageSize for Size4KiB {}

impl PageSize for Size2MiB {
    const SIZE: u64 = Size4KiB::SIZE * 512;
    const SIZE_AS_DEBUG_STR: &'static str = "2MiB";
}

impl NotGiantPageSize for Size2MiB {}

impl PageSize for Size1GiB {
    const SIZE: u64 = Size2MiB::SIZE * 512;
    const SIZE_AS_DEBUG_STR: &'static str = "1GiB";
}

/// A virtual memory page.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Page<S: PageSize = Size4KiB> {
    start_address: VirtAddr,
    size: PhantomData<S>,
}

impl<S: PageSize> Page<S> {
    /// Returns the page that contains the given virtual address.
    #[inline]
    pub fn containing_address(address: VirtAddr) -> Self {
        Page {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    /// Returns the start address of the page.
    #[inline]
    pub fn start_address(self) -> VirtAddr {
        self.start_address
    }

    /// Returns the level 5 page table index of this page.
    #[inline]
    pub fn p5_index(self) -> PageTableIndex {
        self.start_address().p5_index()
    }

    /// Returns the level 4 page table index of this page.
    #[inline]
    pub fn p4_index(self) -> PageTableIndex {
        self.start_address().p4_index()
    }

    /// Returns the level 3 page table index of this page.
    #[inline]
    pub fn p3_index(self) -> PageTableIndex {
        self.start_address().p3_index()
    }

    /// Returns a range of pages, inclusive `end`.
    #[inline]
    pub fn range_inclusive(start: Self, end: Self) -> PageRangeInclusive<S> {
        PageRangeInclusive { start, end }
    }
}

impl<S: NotGiantPageSize> Page<S> {
    /// Returns the level 2 page table index of this page.
    #[inline]
    pub fn p2_index(self) -> PageTableIndex {
        self.start_address().p2_index()
    }
}

impl Page<Size4KiB> {
    /// Returns the level 1 page table index of this page.
    #[inline]
    pub const fn p1_index(self) -> PageTableIndex {
        self.start_address.p1_index()
    }
}

impl<S: PageSize> fmt::Debug for Page<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!(
            "Page[{}]({:#x})",
            S::SIZE_AS_DEBUG_STR,
            self.start_address().as_u64()
        ))
    }
}

impl<S: PageSize> Add<u64> for Page<S> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        Page::containing_address(self.start_address() + rhs * S::SIZE)
    }
}

impl<S: PageSize> AddAssign<u64> for Page<S> {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        *self = *self + rhs;
    }
}

impl<S: PageSize> Sub<u64> for Page<S> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        Page::containing_address(self.start_address() - rhs * S::SIZE)
    }
}

impl<S: PageSize> SubAssign<u64> for Page<S> {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        *self = *self - rhs;
    }
}

impl<S: PageSize> Sub<Self> for Page<S> {
    type Output = u64;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        (self.start_address - rhs.start_address) / S::SIZE
    }
}

/// A range of pages with exclusive upper bound.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PageRange<S: PageSize = Size4KiB> {
    /// The start of the range, inclusive.
    pub start: Page<S>,
    /// The end of the range, exclusive.
    pub end: Page<S>,
}

impl<S: PageSize> Iterator for PageRange<S> {
    type Item = Page<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let page = self.start;
            self.start += 1;
            Some(page)
        } else {
            None
        }
    }
}

impl<S: PageSize> fmt::Debug for PageRange<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PageRange")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

/// A range of pages with inclusive upper bound.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PageRangeInclusive<S: PageSize = Size4KiB> {
    /// The start of the range, inclusive.
    pub start: Page<S>,
    /// The end of the range, inclusive.
    pub end: Page<S>,
}

impl<S: PageSize> Iterator for PageRangeInclusive<S> {
    type Item = Page<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let page = self.start;
            self.start += 1;
            Some(page)
        } else {
            None
        }
    }
}

impl<S: PageSize> fmt::Debug for PageRangeInclusive<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PageRangeInclusive")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

/// The given address was not sufficiently aligned.
#[derive(Debug)]
pub struct AddressNotAligned;

impl fmt::Display for AddressNotAligned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the given address was not sufficiently aligned")
    }
}

/// A physical memory frame.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct PhysFrame<S: PageSize = Size4KiB> {
    pub(crate) start_address: PhysAddr, // TODO: remove when start_address() is const
    size: PhantomData<S>,
}

impl<S: PageSize> PhysFrame<S> {
    /// Returns the frame that starts at the given virtual address.
    ///
    /// Returns an error if the address is not correctly aligned (i.e. is not a valid frame start).
    #[inline]
    pub fn from_start_address(address: PhysAddr) -> Result<Self, AddressNotAligned> {
        if !address.is_aligned(S::SIZE) {
            return Err(AddressNotAligned);
        }
        Ok(PhysFrame::containing_address(address))
    }

    /// Returns the frame that contains the given physical address.
    #[inline]
    pub fn containing_address(address: PhysAddr) -> Self {
        PhysFrame {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    /// Returns the start address of the frame.
    #[inline]
    pub fn start_address(self) -> PhysAddr {
        self.start_address
    }
}

impl<S: PageSize> fmt::Debug for PhysFrame<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!(
            "PhysFrame[{}]({:#x})",
            S::SIZE_AS_DEBUG_STR,
            self.start_address().as_u64()
        ))
    }
}

impl<S: PageSize> Add<u64> for PhysFrame<S> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: u64) -> Self::Output {
        PhysFrame::containing_address(self.start_address() + rhs * S::SIZE)
    }
}

impl<S: PageSize> AddAssign<u64> for PhysFrame<S> {
    #[inline]
    fn add_assign(&mut self, rhs: u64) {
        *self = *self + rhs;
    }
}

impl<S: PageSize> Sub<u64> for PhysFrame<S> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: u64) -> Self::Output {
        PhysFrame::containing_address(self.start_address() - rhs * S::SIZE)
    }
}

impl<S: PageSize> SubAssign<u64> for PhysFrame<S> {
    #[inline]
    fn sub_assign(&mut self, rhs: u64) {
        *self = *self - rhs;
    }
}

impl<S: PageSize> Sub<PhysFrame<S>> for PhysFrame<S> {
    type Output = u64;
    #[inline]
    fn sub(self, rhs: PhysFrame<S>) -> Self::Output {
        (self.start_address - rhs.start_address) / S::SIZE
    }
}

/// An range of physical memory frames, exclusive the upper bound.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PhysFrameRange<S: PageSize = Size4KiB> {
    /// The start of the range, inclusive.
    pub start: PhysFrame<S>,
    /// The end of the range, exclusive.
    pub end: PhysFrame<S>,
}

impl<S: PageSize> Iterator for PhysFrameRange<S> {
    type Item = PhysFrame<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            let frame = self.start;
            self.start += 1;
            Some(frame)
        } else {
            None
        }
    }
}

impl<S: PageSize> fmt::Debug for PhysFrameRange<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PhysFrameRange")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

/// An range of physical memory frames, inclusive the upper bound.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PhysFrameRangeInclusive<S: PageSize = Size4KiB> {
    /// The start of the range, inclusive.
    pub start: PhysFrame<S>,
    /// The start of the range, inclusive.
    pub end: PhysFrame<S>,
}

impl<S: PageSize> Iterator for PhysFrameRangeInclusive<S> {
    type Item = PhysFrame<S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let frame = self.start;
            self.start += 1;
            Some(frame)
        } else {
            None
        }
    }
}

impl<S: PageSize> fmt::Debug for PhysFrameRangeInclusive<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("PhysFrameRangeInclusive")
            .field("start", &self.start)
            .field("end", &self.end)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[aero_test::test]
    pub fn test_page_ranges() {
        let page_size = Size4KiB::SIZE;
        let number = 1000;

        let start_addr = VirtAddr::new(0xdead_beaf);
        let start: Page = Page::containing_address(start_addr);
        let end = start + number;

        let mut range_inclusive = Page::range_inclusive(start, end);
        for i in 0..=number {
            assert_eq!(
                range_inclusive.next(),
                Some(Page::containing_address(start_addr + page_size * i))
            );
        }
        assert_eq!(range_inclusive.next(), None);
    }
}
