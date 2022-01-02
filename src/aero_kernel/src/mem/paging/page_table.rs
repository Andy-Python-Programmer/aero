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

//! Abstractions for page tables and page table entries.

use core::fmt;
use core::ops::{Index, IndexMut};

use super::addr::PhysAddr;
use super::page::{PageSize, PhysFrame, Size4KiB};
use super::{FrameAllocator, MapToError, FRAME_ALLOCATOR};

use bitflags::bitflags;

/// The error returned by the `PageTableEntry::frame` method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrameError {
    /// The entry does not have the `PRESENT` flag set, so it isn't currently mapped to a frame.
    FrameNotPresent,
    /// The entry does have the `HUGE_PAGE` flag set. The `frame` method has a standard 4KiB frame
    /// as return type, so a huge frame can't be returned.
    HugeFrame,
}

/// A 64-bit page table entry.
#[derive(Clone)]
#[repr(transparent)]
pub struct PageTableEntry {
    entry: u64,
}

impl PageTableEntry {
    const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
    // We use the unused flag bits in the page table entry for the
    // page table entry counter.
    const COUNTER_MASK: u64 = 0x7ff0_0000_0000_0000;
    const COUNTER_SHIFT: u64 = 52;
    const FLAGS_MASK: u64 = 0x80000000000001ff;

    /// Creates an unused page table entry.
    pub const fn new() -> Self {
        PageTableEntry { entry: 0 }
    }

    /// Returns whether this entry is zero.
    pub const fn is_unused(&self) -> bool {
        self.entry == 0
    }

    /// Sets this entry to zero.
    pub fn set_unused(&mut self) {
        self.entry = 0;
    }

    /// Returns the flags of this entry.
    pub const fn flags(&self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.entry)
    }

    pub fn unref_vm_frame(&self) -> bool {
        if self.addr() != PhysAddr::new(0x00) {
            if let Some(vm_frame) = self.addr().as_vm_frame() {
                vm_frame.dec_ref_count();

                let count = vm_frame.ref_count();

                if count == 0 {
                    // No references to this frame, deallocate it.
                    unsafe {
                        FRAME_ALLOCATOR.deallocate_frame(
                            PhysFrame::<Size4KiB>::containing_address(self.addr()),
                        );
                    }

                    return true;
                }
            }
        }

        false
    }

    pub fn ref_vm_frame(&self) {
        if self.addr() != PhysAddr::new(0x00) {
            if let Some(vm_frame) = self.addr().as_vm_frame() {
                vm_frame.inc_ref_count();
            }
        }
    }

    /// Returns the physical address mapped by this entry, might be zero.
    pub fn addr(&self) -> PhysAddr {
        PhysAddr::new(self.entry & Self::ADDRESS_MASK)
    }

    /// Returns the unused flag bits in the page table entry for the
    /// page table entry counter.
    pub fn get_entry_count(&self) -> u64 {
        (self.entry & Self::COUNTER_MASK) >> Self::COUNTER_SHIFT
    }

    /// Sets the page table entry counter to the given `count`.
    pub fn set_entry_count(&mut self, count: u64) {
        self.entry = (self.entry & !Self::COUNTER_MASK) | (count << Self::COUNTER_SHIFT);
    }

    /// Increments the page table entry counter by one.
    pub fn inc_entry_count(&mut self) {
        let entry_count = self.get_entry_count();
        self.set_entry_count(entry_count + 1);
    }

    /// Decrements the page table entry counter by one.
    pub fn dec_entry_count(&mut self) {
        let entry_count = self.get_entry_count();
        assert!(entry_count != 0);

        self.set_entry_count(entry_count - 1);
    }

    /// Returns the physical frame mapped by this entry.
    ///
    /// Returns the following errors:
    ///
    /// - `FrameError::FrameNotPresent` if the entry doesn't have the `PRESENT` flag set.
    /// - `FrameError::HugeFrame` if the entry has the `HUGE_PAGE` flag set (for huge pages the
    ///    `addr` function must be used)
    pub fn frame(&self) -> Result<PhysFrame, FrameError> {
        if !self.flags().contains(PageTableFlags::PRESENT) {
            Err(FrameError::FrameNotPresent)
        } else if self.flags().contains(PageTableFlags::HUGE_PAGE) {
            Err(FrameError::HugeFrame)
        } else {
            Ok(PhysFrame::containing_address(self.addr()))
        }
    }

    /// Map the entry to the specified physical address with the specified flags.
    pub fn set_addr(&mut self, addr: PhysAddr, flags: PageTableFlags) {
        assert!(addr.is_aligned(Size4KiB::SIZE));

        let ref_pp = self.addr() != addr;

        if ref_pp {
            self.unref_vm_frame();
        }

        self.entry &= !Self::ADDRESS_MASK;
        self.entry |= addr.as_u64();

        self.set_flags(flags);

        if ref_pp {
            self.ref_vm_frame();
        }
    }

    /// Map the entry to the specified physical frame with the specified flags.
    pub fn set_frame(&mut self, frame: PhysFrame, flags: PageTableFlags) {
        assert!(!flags.contains(PageTableFlags::HUGE_PAGE));
        self.set_addr(frame.start_address(), flags)
    }

    /// Sets the flags of this entry.
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        self.entry &= !Self::FLAGS_MASK;
        self.entry |= flags.bits();
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_struct("PageTableEntry");
        f.field("addr", &self.addr());
        f.field("flags", &self.flags());
        f.finish()
    }
}

bitflags! {
    /// Possible flags for a page table entry.
    pub struct PageTableFlags: u64 {
        /// Specifies whether the mapped frame or page table is loaded in memory.
        const PRESENT =         1;
        /// Controls whether writes to the mapped frames are allowed.
        ///
        /// If this bit is unset in a level 1 page table entry, the mapped frame is read-only.
        /// If this bit is unset in a higher level page table entry the complete range of mapped
        /// pages is read-only.
        const WRITABLE =        1 << 1;
        /// Controls whether accesses from userspace (i.e. ring 3) are permitted.
        const USER_ACCESSIBLE = 1 << 2;
        /// If this bit is set, a “write-through” policy is used for the cache, else a “write-back”
        /// policy is used.
        const WRITE_THROUGH =   1 << 3;
        /// Disables caching for the pointed entry is cacheable.
        const NO_CACHE =        1 << 4;
        /// Set by the CPU when the mapped frame or page table is accessed.
        const ACCESSED =        1 << 5;
        /// Set by the CPU on a write to the mapped frame.
        const DIRTY =           1 << 6;
        /// Specifies that the entry maps a huge frame instead of a page table. Only allowed in
        /// P2 or P3 tables.
        const HUGE_PAGE =       1 << 7;
        /// Indicates that the mapping is present in all address spaces, so it isn't flushed from
        /// the TLB on an address space switch.
        const GLOBAL =          1 << 8;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_9 =           1 << 9;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_10 =          1 << 10;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_11 =          1 << 11;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_52 =          1 << 52;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_53 =          1 << 53;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_54 =          1 << 54;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_55 =          1 << 55;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_56 =          1 << 56;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_57 =          1 << 57;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_58 =          1 << 58;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_59 =          1 << 59;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_60 =          1 << 60;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_61 =          1 << 61;
        /// Available to the OS, can be used to store additional data, e.g. custom flags.
        const BIT_62 =          1 << 62;
        /// Forbid code execution from the mapped frames.
        ///
        /// Can be only used when the no-execute page protection feature is enabled in the EFER
        /// register.
        const NO_EXECUTE =      1 << 63;
    }
}

/// The number of entries in a page table.
const ENTRY_COUNT: usize = 512;

/// Represents a page table.
///
/// Always page-sized.
///
/// This struct implements the `Index` and `IndexMut` traits, so the entries can be accessed
/// through index operations. For example, `page_table[15]` returns the 15th page table entry.
///
/// Note that while this type implements [`Clone`], the users must be careful not to introduce
/// mutable aliasing by using the cloned page tables.
#[repr(align(4096))]
#[repr(C)]
#[derive(Clone)]
pub struct PageTable {
    pub(super) entries: [PageTableEntry; ENTRY_COUNT],
}

impl PageTable {
    /// Creates an empty page table.
    #[inline]
    pub const fn new() -> Self {
        const EMPTY: PageTableEntry = PageTableEntry::new();
        PageTable {
            entries: [EMPTY; ENTRY_COUNT],
        }
    }

    /// Clears all entries.
    #[inline]
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }

    pub fn for_entries_mut(
        &mut self,
        flags: PageTableFlags,
        mut fun: impl FnMut(
            usize,
            &mut PageTableEntry,
            &mut PageTable,
        ) -> Result<(), MapToError<Size4KiB>>,
    ) -> Result<(), MapToError<Size4KiB>> {
        self.entries
            .iter_mut()
            .enumerate()
            .filter(|(_, e)| e.flags().contains(flags))
            .try_for_each(|(idx, e)| {
                let table = unsafe {
                    let addr = crate::PHYSICAL_MEMORY_OFFSET + e.addr().as_u64();
                    &mut *addr.as_mut_ptr::<PageTable>()
                };

                fun(idx, e, table)
            })
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

impl Index<PageTableIndex> for PageTable {
    type Output = PageTableEntry;

    #[inline]
    fn index(&self, index: PageTableIndex) -> &Self::Output {
        &self.entries[usize::from(index)]
    }
}

impl IndexMut<PageTableIndex> for PageTable {
    #[inline]
    fn index_mut(&mut self, index: PageTableIndex) -> &mut Self::Output {
        &mut self.entries[usize::from(index)]
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for PageTable {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.entries[..].fmt(f)
    }
}

/// A 9-bit index into a page table.
///
/// Can be used to select one of the 512 entries of a page table.
///
/// Guaranteed to only ever contain 0..512.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageTableIndex(u16);

impl PageTableIndex {
    /// Creates a new index from the given `u16`. Panics if the given value is >=512.
    #[inline]
    pub fn new(index: u16) -> Self {
        assert!(usize::from(index) < ENTRY_COUNT);
        Self(index)
    }

    /// Creates a new index from the given `u16`. Throws away bits if the value is >=512.
    #[inline]
    pub const fn new_truncate(index: u16) -> Self {
        Self(index % ENTRY_COUNT as u16)
    }
}

impl From<PageTableIndex> for u16 {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        index.0
    }
}

impl From<PageTableIndex> for u32 {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        u32::from(index.0)
    }
}

impl From<PageTableIndex> for u64 {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        u64::from(index.0)
    }
}

impl From<PageTableIndex> for usize {
    #[inline]
    fn from(index: PageTableIndex) -> Self {
        usize::from(index.0)
    }
}

/// A 12-bit offset into a 4KiB Page.
///
/// This type is returned by the `VirtAddr::page_offset` method.
///
/// Guaranteed to only ever contain 0..4096.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageOffset(u16);

impl PageOffset {
    /// Creates a new offset from the given `u16`. Panics if the passed value is >=4096.
    #[inline]
    pub fn new(offset: u16) -> Self {
        assert!(offset < (1 << 12));
        Self(offset)
    }

    /// Creates a new offset from the given `u16`. Throws away bits if the value is >=4096.
    #[inline]
    pub const fn new_truncate(offset: u16) -> Self {
        Self(offset % (1 << 12))
    }
}

impl From<PageOffset> for u16 {
    #[inline]
    fn from(offset: PageOffset) -> Self {
        offset.0
    }
}

impl From<PageOffset> for u32 {
    #[inline]
    fn from(offset: PageOffset) -> Self {
        u32::from(offset.0)
    }
}

impl From<PageOffset> for u64 {
    #[inline]
    fn from(offset: PageOffset) -> Self {
        u64::from(offset.0)
    }
}

impl From<PageOffset> for usize {
    #[inline]
    fn from(offset: PageOffset) -> Self {
        usize::from(offset.0)
    }
}
