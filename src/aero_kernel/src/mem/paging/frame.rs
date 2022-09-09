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

use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::vec::Vec;
use limine::{LimineMemmapEntry, LimineMemoryMapEntryType, NonNullPtr};
use spin::Once;

use super::mapper::*;
use super::page::*;

use super::addr::PhysAddr;

use crate::mem::paging::align_up;
use crate::utils::bitmap::Bitmap;
use crate::utils::sync::Mutex;

static BUDDY_SIZE: [u64; 3] = [Size4KiB::SIZE, Size4KiB::SIZE * 4, Size2MiB::SIZE];

pub struct LockedFrameAllocator(Once<Mutex<GlobalFrameAllocator>>);

impl LockedFrameAllocator {
    /// Constructs a new uninitialized and locked version of the global frame
    /// allocator.
    pub(super) const fn new_uninit() -> Self {
        Self(Once::new())
    }

    /// Initializes the inner locked global frame allocator.
    pub(super) fn init(&self, memory_map: &mut [NonNullPtr<LimineMemmapEntry>]) {
        self.0
            .call_once(|| Mutex::new(GlobalFrameAllocator::new(memory_map)));
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&self) -> Option<PhysFrame<Size4KiB>> {
        // let caller = core::panic::Location::caller();
        // log::debug!("allocation request of 4KiB by {:?}", caller);

        self.0
            .get()
            .map(|m| {
                m.lock()
                    .allocate_frame_inner(0)
                    .map(|f| PhysFrame::containing_address(f))
            })
            .unwrap_or(None)
            .map(|frame| {
                frame.as_slice_mut().fill(0);
                frame
            })
    }

    #[track_caller]
    fn deallocate_frame(&self, frame: PhysFrame<Size4KiB>) {
        // let caller = core::panic::Location::caller();
        // log::debug!("deallocation request of 4KiB by {:?}", caller);

        // self.0
        //     .get()
        //     .map(|m| m.lock().deallocate_frame_inner(frame.start_address(), 0))
        //     .unwrap_or(());
    }
}

unsafe impl FrameAllocator<Size2MiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&self) -> Option<PhysFrame<Size2MiB>> {
        // let caller = core::panic::Location::caller();
        // log::debug!("allocation request of 2MiB by {:?}", caller);

        self.0
            .get()
            .map(|m| {
                m.lock()
                    .allocate_frame_inner(2)
                    .map(|f| PhysFrame::containing_address(f))
            })
            .unwrap_or(None)
            .map(|frame| {
                frame.as_slice_mut().fill(0);
                frame
            })
    }

    #[track_caller]
    fn deallocate_frame(&self, frame: PhysFrame<Size2MiB>) {
        // let caller = core::panic::Location::caller();
        // log::debug!("deallocation request of 2MiB by {:?}", caller);

        self.0
            .get()
            .map(|m| m.lock().deallocate_frame_inner(frame.start_address(), 2))
            .unwrap_or(());
    }
}

struct RangeMemoryIter<'a> {
    iter: core::slice::Iter<'a, NonNullPtr<LimineMemmapEntry>>,

    cursor_base: PhysAddr,
    cursor_end: PhysAddr,
}

impl<'a> Iterator for RangeMemoryIter<'a> {
    type Item = MemoryRange;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor_base >= self.cursor_end {
            if let Some(entry) = loop {
                // We need to find out the next useable memory range from
                // the memory map and set the cursor to the start of it.
                let next = self.iter.next()?;

                if next.typ == LimineMemoryMapEntryType::Usable {
                    break Some(next);
                }
            } {
                self.cursor_base = PhysAddr::new(entry.base).align_up(Size4KiB::SIZE);
                self.cursor_end = PhysAddr::new(entry.base + entry.len);
            } else {
                // We reached the end of the memory map.
                return None;
            }
        }

        let typee = MemoryRangeType::Usable;

        let range = MemoryRange {
            addr: self.cursor_base,
            size: self.cursor_end - self.cursor_base,
            typee,
        };

        self.cursor_base = self.cursor_end.align_up(Size4KiB::SIZE);
        Some(range)
    }
}

#[repr(usize)]
pub enum BuddyOrdering {
    Size4KiB = 0,
    Size8KiB = 2,
}

pub fn pmm_alloc(order: BuddyOrdering) -> PhysAddr {
    let order = order as usize;
    debug_assert!(order <= BUDDY_SIZE.len());

    let addr = super::FRAME_ALLOCATOR
        .0
        .get()
        .expect("pmm: frame allocator not initialized")
        .lock()
        .allocate_frame_inner(order)
        .expect("pmm: out of memory");

    let virt = addr.as_hhdm_virt();

    let fill_size = BUDDY_SIZE[order] as usize;
    let slice = unsafe { core::slice::from_raw_parts_mut(virt.as_mut_ptr::<u8>(), fill_size) };

    // We always zero out memory for security reasons.
    slice.fill(0x00);

    addr
}

#[derive(Debug)]
struct MemoryRange {
    addr: PhysAddr,
    size: u64,
    typee: MemoryRangeType,
}

#[derive(Debug, PartialEq)]
enum MemoryRangeType {
    Usable,
}

struct BootAlloc {
    memory_ranges: Mutex<&'static mut [MemoryRange]>,
}

impl BootAlloc {
    fn new(memory_ranges: &'static mut [MemoryRange]) -> Self {
        Self {
            memory_ranges: Mutex::new(memory_ranges),
        }
    }

    fn allocate_inner(&self, size: usize) -> *mut u8 {
        let size = align_up(size as u64, Size4KiB::SIZE);
        for range in self.memory_ranges.lock().iter_mut().rev() {
            if range.size >= size {
                let addr = range.addr;

                range.addr += size;
                range.size -= size;

                return addr.as_hhdm_virt().as_mut_ptr();
            }
        }

        unreachable!("pmm: bootstrap allocator is out of memory")
    }
}

#[derive(Debug, Clone)]
struct BootAllocRef {
    inner: *const BootAlloc,
}

impl BootAllocRef {
    fn new(inner: &BootAlloc) -> Self {
        Self {
            inner: inner as *const _,
        }
    }

    fn get_inner(&self) -> &BootAlloc {
        unsafe { &*self.inner }
    }
}

unsafe impl Allocator for BootAllocRef {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let this = self.get_inner();

        let aligned_size = align_up(layout.size() as _, layout.align() as _) as usize;
        let ptr = this.allocate_inner(aligned_size);

        // SAFETY: `allocate_inner` is garunteed to return a valid, non-null pointer.
        let ptr = unsafe { NonNull::new_unchecked(ptr) };
        Ok(NonNull::slice_from_raw_parts(ptr, aligned_size))
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        unreachable!("pmm: bootstrap allocator cannot deallocate")
    }
}

unsafe impl Send for BootAllocRef {}

static VM_FRAMES: Once<Vec<VmFrame>> = Once::new();

/// Buddy allocator combines power-of-two allocator with free buffer
/// coalescing.
///
/// ## Overview
///
/// Overview of the buddy allocation algorithm:
///
/// * Memory is broken up into large blocks of pages where each block
///   is a power of two number of pages.
///
/// * If a block of the desired size is not available, a larger block is
///   broken up in half and the two blocks are marked as buddies then one half
///   is used for the allocation and the other half is marked free.
///
/// * The blocks are continuously halved as necessary until a block of the
///   desired size is available.
///
/// * When a block is later freed, the buddy is examined and the two coalesced
///   if it is free.
pub struct GlobalFrameAllocator {
    buddies: [Bitmap<BootAllocRef>; 3],
    free: [usize; 3],

    base: PhysAddr,
    end: PhysAddr,
}

impl GlobalFrameAllocator {
    /// Create a new global frame allocator from the memory map provided by the bootloader.
    fn new(memory_map: &mut [NonNullPtr<LimineMemmapEntry>]) -> Self {
        // Find a memory map entry that is big enough to fit all of the items in
        // range memory iter.
        let requested_size = (core::mem::size_of::<MemoryRange>() * memory_map.len()) as u64;
        let mut region = None;

        for i in 0..memory_map.len() {
            let entry = &mut memory_map[i];

            // Make sure that the memory map entry is marked as usable.
            if entry.typ != LimineMemoryMapEntryType::Usable {
                continue;
            }

            // Found a big enough memory map entry.
            if entry.len >= requested_size {
                let base = entry.base;

                entry.base += requested_size;
                entry.len -= requested_size;

                region = Some(PhysAddr::new(base));

                break;
            }
        }

        let mut iter = memory_map.iter();

        let cursor = iter
            .next()
            .expect("stivale2: unexpected end of the memory map");

        let ranges = unsafe {
            let virt_addr = region.expect("stivale2: out of memory").as_hhdm_virt();

            core::slice::from_raw_parts_mut::<MemoryRange>(
                virt_addr.as_mut_ptr(),
                requested_size as usize,
            )
        };

        let range_iter = RangeMemoryIter {
            iter,

            cursor_base: PhysAddr::new(cursor.base),
            cursor_end: PhysAddr::new(cursor.base + cursor.len),
        };

        // Lets goo! Now lets initialize the bootstrap allocator so we can initialize
        // our efficient buddy allocator. We need a seperate allocator since some computers
        // such as Macs have a shitload of memory map entries so, we cannt assume the amount
        // of maximum mmap entries and allocate space for it on the stack instead. God damn it.
        let mut i = 0;

        for range in range_iter {
            ranges[i] = range;
            i += 1;
        }

        let base = ranges[0].addr;
        let end = ranges[i - 1].addr + ranges[i - 1].size;

        let bootstrapper = BootAlloc::new(&mut ranges[..i]);
        let bref = BootAllocRef::new(&bootstrapper);

        let mut this = Self {
            base,
            end,

            buddies: [
                Bitmap::empty(bref.clone()),
                Bitmap::empty(bref.clone()),
                Bitmap::empty(bref.clone()),
            ],
            free: [0; 3],
        };

        let size = this.end - this.base;

        // Allocate the buddies using prealloc:
        for (i, bsize) in BUDDY_SIZE.iter().enumerate() {
            let chunk = size / bsize;
            this.buddies[i] = Bitmap::new_in(bref.clone(), chunk as usize);
        }

        for region in bref.get_inner().memory_ranges.lock().iter() {
            if region.typee == MemoryRangeType::Usable {
                this.insert_range(region.addr, region.addr + region.size);
            }
        }

        this
    }

    fn frame_count(&self) -> usize {
        (self.end.as_u64() / Size4KiB::SIZE) as usize
    }

    /// Find the perfect buddy order for the provided address range.
    fn find_order(&self, address: PhysAddr, chunk_size: u64) -> usize {
        for order in (0..BUDDY_SIZE.len()).rev() {
            let size = BUDDY_SIZE[order];

            // Too big...
            if size > chunk_size {
                continue;
            }

            let mask = BUDDY_SIZE[order] - 1;

            if mask & address.as_u64() != 0 {
                continue;
            } else {
                return order;
            }
        }

        return 0;
    }

    fn get_bit_idx(&self, addr: PhysAddr, order: usize) -> usize {
        let offset = addr - self.base;
        (offset / BUDDY_SIZE[order]) as usize
    }

    fn set_bit(&mut self, addr: PhysAddr, order: usize) -> bool {
        let idx = self.get_bit_idx(addr, order);

        let buddy = &mut self.buddies[order];
        let change = !buddy.is_set(idx);

        if change {
            buddy.set(idx, true);
            self.free[order] += 1;
        }

        change
    }

    #[cfg(test)]
    fn is_free(&self, addr: PhysAddr, order: usize) -> bool {
        let idx = self.get_bit_idx(addr, order);

        let buddy = &self.buddies[order];
        buddy.is_set(idx)
    }

    /// Inserts the provided memory range.
    fn insert_range(&mut self, base: PhysAddr, end: PhysAddr) {
        let mut remaning = end - base;
        let mut current = base;

        while remaning > 0 {
            let order = self.find_order(current, remaning);
            let size = BUDDY_SIZE[order];

            self.set_bit(current, order);

            current += size;
            remaning -= size;
        }
    }

    /// Finds a free chunk with the provided `order`.
    fn find_free(&mut self, order: usize) -> Option<PhysAddr> {
        let buddy = &mut self.buddies[order];
        let first_free = buddy.find_first_set()?;

        buddy.set(first_free, false);
        self.free[order] -= 1;

        Some(self.base.align_up(BUDDY_SIZE[order]) + (BUDDY_SIZE[order] * first_free as u64))
    }

    fn clear_bit(&mut self, addr: PhysAddr, order: usize) -> bool {
        let idx = self.get_bit_idx(addr, order);

        let buddy = &mut self.buddies[order];
        let change = buddy.is_set(idx) == true;

        if change {
            buddy.set(idx, false);
            self.free[order] -= 1;
        }

        change
    }

    fn get_buddy(&self, addr: PhysAddr, order: usize) -> PhysAddr {
        let size = BUDDY_SIZE[order];
        let base = addr.align_down(size * 2);

        if base == addr {
            addr + size
        } else {
            base
        }
    }

    fn deallocate_frame_inner(&mut self, mut addr: PhysAddr, mut order: usize) {
        while order < BUDDY_SIZE.len() {
            if order < BUDDY_SIZE.len() - 1 {
                let buddy = self.get_buddy(addr, order);

                if self.clear_bit(buddy, order) {
                    addr = core::cmp::min(addr, buddy);
                    order += 1;
                } else {
                    self.set_bit(addr, order);
                    break;
                }
            } else {
                self.set_bit(addr, order);
                break;
            }
        }
    }

    fn allocate_frame_inner(&mut self, order: usize) -> Option<PhysAddr> {
        let size = BUDDY_SIZE[order];

        // Loop through the list of buddies until we can find one that can give us
        // the requested memory.
        for (i, &bsize) in BUDDY_SIZE[order..].iter().enumerate() {
            let i = i + order;

            if self.free[i] > 0 {
                let result = self.find_free(i)?;
                let mut remaning = bsize - size;

                if remaning > 0 {
                    for j in (0..=i).rev() {
                        let sizee = BUDDY_SIZE[j];

                        while remaning >= sizee {
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

pub fn init_vm_frames() {
    VM_FRAMES.call_once(|| {
        let frame_count = super::FRAME_ALLOCATOR
            .0
            .get()
            .unwrap()
            .lock_irq()
            .frame_count();

        let mut frames = Vec::<VmFrame>::new();
        frames.resize_with(frame_count, VmFrame::new);

        frames
    });
}

pub fn get_vm_frames() -> Option<&'static Vec<VmFrame>> {
    VM_FRAMES.get()
}

pub struct VmFrame {
    ref_count: AtomicUsize,
}

impl VmFrame {
    fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(0),
        }
    }

    pub fn dec_ref_count(&self) {
        let ref_count = self.ref_count.load(Ordering::SeqCst);

        if ref_count != 0 {
            self.ref_count.store(ref_count - 1, Ordering::SeqCst);
        }
    }

    pub fn inc_ref_count(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    use crate::mem::AddressSpace;

    #[test]
    fn vm_frame_ref_count() {
        let mut address_space = AddressSpace::this();
        let mut offset_table = address_space.offset_page_table();

        let frame: PhysFrame = FRAME_ALLOCATOR.allocate_frame().unwrap();

        assert!(!FRAME_ALLOCATOR
            .0
            .get()
            .unwrap()
            .lock()
            .is_free(frame.start_address(), 0));

        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(0xcafebabedea));

        let vm_frame = frame.start_address().as_vm_frame().unwrap();

        // The frame is not mapped yet, so the ref count should be 0.
        assert_eq!(vm_frame.ref_count(), 0);

        assert!(!FRAME_ALLOCATOR
            .0
            .get()
            .unwrap()
            .lock()
            .is_free(frame.start_address(), 0));

        unsafe { offset_table.map_to(page, frame, PageTableFlags::PRESENT) }
            .unwrap()
            .flush();

        // We just mapped the frame to `0xcafebabe` so the ref count should be 1.
        assert_eq!(vm_frame.ref_count(), 1);

        offset_table.unmap(page).unwrap().1.flush();

        // We just unmapped the frame from `0xcafebabe` so the ref count should be 0 and
        // the frame should be deallocated.
        assert_eq!(vm_frame.ref_count(), 0);

        assert!(FRAME_ALLOCATOR
            .0
            .get()
            .unwrap()
            .lock()
            .is_free(frame.start_address(), 0));
    }
}
