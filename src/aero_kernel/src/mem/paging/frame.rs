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

use bit_field::BitField;
use spin::Once;
use stivale_boot::v2::{StivaleMemoryMapEntryType, StivaleMemoryMapIter, StivaleMemoryMapTag};

use super::mapper::*;
use super::page::*;

use super::addr::PhysAddr;

use crate::utils::sync::Mutex;

// TODO: There might be a case where prealloc runs out of memory even if
// the memory map length < 256. We need to find a better solution rather then
// just extending the capacity. Like a bootstrap allocator that is used by the buddy
// allocator to allocate space for its buddies.
const PREALLOC_CAPACITY: usize = 4096 * 256;
const BUDDY_BITS: u64 = (core::mem::size_of::<usize>() * 8) as u64;

static BUDDY_SIZE: [u64; 2] = [Size4KiB::SIZE, Size2MiB::SIZE];
static PREALLOC: Mutex<PreAlloc> = Mutex::new(PreAlloc::new());

pub struct LockedFrameAllocator(Once<Mutex<GlobalFrameAllocator>>);

impl LockedFrameAllocator {
    /// Constructs a new uninitialized and locked version of the global frame
    /// allocator.
    pub(super) const fn new_uninit() -> Self {
        Self(Once::new())
    }

    /// Initializes the inner locked global frame allocator.
    pub(super) fn init(
        &self,
        memory_map: &'static StivaleMemoryMapTag,
        kernel_base: PhysAddr,
        kernel_end: PhysAddr,
    ) {
        self.0.call_once(|| {
            Mutex::new(GlobalFrameAllocator::new(
                memory_map,
                kernel_base,
                kernel_end,
            ))
        });
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.0
            .get()
            .map(|m| {
                m.lock()
                    .allocate_frame_inner(0)
                    .map(|f| PhysFrame::containing_address(f))
            })
            .unwrap_or(None)
    }
}

unsafe impl FrameAllocator<Size2MiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size2MiB>> {
        self.0
            .get()
            .map(|m| {
                m.lock()
                    .allocate_frame_inner(1)
                    .map(|f| PhysFrame::containing_address(f))
            })
            .unwrap_or(None)
    }
}

pub struct RangeMemoryIter {
    iter: StivaleMemoryMapIter<'static>,

    kernel_base: PhysAddr,
    kernel_end: PhysAddr,

    cursor_base: PhysAddr,
    cursor_end: PhysAddr,
}

impl RangeMemoryIter {
    /// Helper function that returns [`true`] if the provided range intersects
    /// between the current cursor base and the cursor end addresses.
    fn intersects_in_range(&self, base: PhysAddr, end: PhysAddr) -> bool {
        (self.cursor_base <= end && self.cursor_end <= end)
            && (base <= self.cursor_base && base <= self.cursor_end)
    }

    fn cursor_align_up(&mut self) -> PhysAddr {
        if self.cursor_base < self.kernel_base {
            self.cursor_base = self.kernel_base;
        }

        if self.intersects_in_range(self.kernel_base, self.kernel_end) {
            self.kernel_end
        } else {
            self.cursor_base
        }
    }

    fn cursor_align_down(&self) -> Option<PhysAddr> {
        if self.cursor_base >= self.cursor_end {
            return None;
        }

        if self.kernel_end < self.cursor_end && self.kernel_base >= self.cursor_base {
            Some(self.kernel_base)
        } else {
            Some(self.cursor_end)
        }
    }
}

impl Iterator for RangeMemoryIter {
    type Item = (PhysAddr, u64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor_base >= self.cursor_end {
            if let Some(entry) = loop {
                // We need to find out the next useable memory range from
                // the memory map and set the cursor to the start of it.
                let next = self.iter.next()?;

                if next.entry_type == StivaleMemoryMapEntryType::Usable {
                    break Some(next);
                }
            } {
                self.cursor_base = PhysAddr::new(entry.base).align_up(Size4KiB::SIZE);
                self.cursor_end = PhysAddr::new(entry.base + entry.length);
            } else {
                // We reached the end of the memory map.
                return None;
            }
        }

        self.cursor_base = self.cursor_align_up().align_up(Size4KiB::SIZE);

        if let Some(end) = self.cursor_align_down() {
            let range = Some((self.cursor_base, end - self.cursor_base));

            self.cursor_base = end.align_up(Size4KiB::SIZE);
            range
        } else {
            self.next()
        }
    }
}

/// Prealloc is used as the bootstrap allocator used to allocate the buddies
/// map for the global frame allocator. Its just a simple bump allocator.
#[repr(align(4096))]
struct PreAlloc {
    space: [u8; PREALLOC_CAPACITY],
    next: usize,
}

impl PreAlloc {
    /// Constructs a new prealloc bump allocator.
    const fn new() -> Self {
        Self {
            space: [0; PREALLOC_CAPACITY],
            next: 0x00,
        }
    }
}

/// Allocates chunk of memory with the provided `size` and return a pointer
/// to it.
fn prealloc(size: usize) -> *mut u8 {
    let mut this = PREALLOC.lock();

    if this.next + size > PREALLOC_CAPACITY {
        panic!("prealloc: out of memory")
    }

    let ptr = unsafe { this.space.as_mut_ptr().offset(this.next as isize) };
    this.next += size;

    ptr
}

pub struct GlobalFrameAllocator {
    buddies: [&'static mut [u64]; 2],
    free: [usize; 2],

    base: PhysAddr,
    end: PhysAddr,
}

impl GlobalFrameAllocator {
    /// Create a new global frame allocator from the memory map provided by the bootloader.
    fn new(
        memory_map: &'static StivaleMemoryMapTag,
        kernel_base: PhysAddr,
        kernel_end: PhysAddr,
    ) -> Self {
        let mut iter = memory_map.iter();
        let cursor = iter
            .next()
            .expect("stivale2: unexpected end of the memory map");

        let range_iter = RangeMemoryIter {
            iter,

            kernel_base,
            kernel_end,

            cursor_base: PhysAddr::new(cursor.base),
            cursor_end: PhysAddr::new(cursor.base + cursor.length),
        };

        // We hardcode the max memory map entries to 256. Only macs have a shitload of
        // memory map entries > 256. Apple momemnt :^)
        let mut ranges = [(PhysAddr::new(0x00), 0x00); 256];
        let mut i = 0;

        for (addr, size) in range_iter {
            ranges[i] = (addr, size);
            i += 1;
        }

        let base = ranges[0].0;
        let end = ranges[i - 1].0 + ranges[i - 1].1;

        let mut this = Self {
            base,
            end,

            buddies: [&mut [], &mut []],
            free: [0; 2],
        };

        let size = this.end - this.base;

        // Allocate the buddies using prealloc:
        for (i, bsize) in BUDDY_SIZE.iter().enumerate() {
            let chunk = ((size / bsize) + BUDDY_BITS - 1) / BUDDY_BITS;
            let chunk_size = chunk * 8;

            let chunk_ptr = prealloc(chunk_size as usize) as *mut u64;
            let chunk_slice = unsafe { core::slice::from_raw_parts_mut(chunk_ptr, chunk as usize) };

            chunk_slice.fill(0x00);
            this.buddies[i] = chunk_slice;
        }

        for &(base, length) in ranges[..i].iter() {
            this.insert_range(base, base + length);
        }

        this
    }

    /// Find the perfect buddy ordering for the provided address range.
    fn find_ordering(&self, address: PhysAddr, chunk_size: u64) -> usize {
        for ordering in (0..BUDDY_SIZE.len()).rev() {
            let size = BUDDY_SIZE[ordering];

            // Too big...
            if size > chunk_size {
                continue;
            }

            let mask = BUDDY_SIZE[ordering] - 1;

            if mask & address.as_u64() != 0 {
                continue;
            } else {
                return ordering;
            }
        }

        return 0;
    }

    /// Helper function that translates a address to it's part in the map. This
    /// function returns a tuple of (index, bit) where index is the index on the
    /// `u64` array and `bit` is the bit over the `u64`.
    fn get_byte_bit(&self, addr: PhysAddr, order: usize) -> (u64, u64) {
        let offset = addr - self.base;
        let id = offset / BUDDY_SIZE[order];

        (id / BUDDY_BITS, id % BUDDY_BITS)
    }

    fn set_bit(&mut self, address: PhysAddr, ordering: usize) -> bool {
        let (byte, bit) = self.get_byte_bit(address, ordering);

        let chunk = &mut self.buddies[ordering][byte as usize];
        let change = (*chunk).get_bit(bit as usize) == false;

        if change {
            (*chunk).set_bit(bit as usize, true);
            self.free[ordering] += 1;
        }

        change
    }

    /// Inserts the provided memory range.
    fn insert_range(&mut self, base: PhysAddr, end: PhysAddr) {
        let mut remaning = end - base;
        let mut current = base;

        while remaning > 0 {
            let ordering = self.find_ordering(current, remaning);
            let size = BUDDY_SIZE[ordering];

            self.set_bit(current, ordering);

            current += size;
            remaning -= size;
        }
    }

    /// Finds a free chunk with the provided `ordering`.
    fn find_free(&mut self, ordering: usize) -> Option<PhysAddr> {
        for (i, chunk) in self.buddies[ordering].iter_mut().enumerate() {
            let mut chunk_value = *chunk;

            if chunk_value != 0 {
                let mut bit = 0;

                while !chunk_value.get_bit(0) {
                    chunk_value >>= 1;
                    bit += 1;
                }

                (*chunk).set_bit(bit, false);
                self.free[ordering] -= 1;

                return Some(
                    self.base.align_up(BUDDY_SIZE[ordering])
                        + (BUDDY_SIZE[ordering] * BUDDY_BITS * i as u64)
                        + BUDDY_SIZE[ordering] * bit as u64,
                );
            }
        }

        None
    }

    fn allocate_frame_inner(&mut self, ordering: usize) -> Option<PhysAddr> {
        let size = BUDDY_SIZE[ordering];

        // Loop through the list of buddies until we can find one that can give us
        // the requested memory.
        for (i, &bsize) in BUDDY_SIZE[ordering..].iter().enumerate() {
            let i = i + ordering;

            if self.free[i] > 0 {
                let result = self.find_free(i)?;
                let mut remaning = bsize - size;

                if remaning > 0 {
                    for j in (0..=i).rev() {
                        let sizee = BUDDY_SIZE[j];

                        if remaning >= sizee {
                            self.set_bit(result + (remaning - sizee) + size, j);
                            remaning -= sizee;
                        }
                    }
                }

                return Some(result);
            }
        }

        None
    }
}
