/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use aero_boot::*;

use spin::{Mutex, Once};
use x86_64::{structures::paging::*, PhysAddr};

pub struct LockedFrameAllocator(Once<Mutex<GlobalFrameAllocator>>);

impl LockedFrameAllocator {
    /// Constructs a new uninitialized and locked version of the global frame
    /// allocator.
    pub(super) const fn new_uninit() -> Self {
        Self(Once::new())
    }

    /// Initializes the inner locked global frame allocator.
    pub(super) fn init(&self, memory_map: &'static MemoryRegions) {
        self.0
            .call_once(|| Mutex::new(GlobalFrameAllocator::new(memory_map)));
    }

    pub fn get_frame_type(&self, frame: PhysFrame) -> Option<MemoryRegionType> {
        if let Some(ref mut allocator) = self.0.get() {
            allocator.lock().get_frame_type(frame)
        } else {
            None
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if let Some(ref mut allocator) = self.0.get() {
            allocator.lock().allocate_frame()
        } else {
            None
        }
    }
}

pub struct GlobalFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl GlobalFrameAllocator {
    /// Create a new global frame allocator from the memory map provided by the bootloader.
    fn new(memory_map: &'static MemoryRegions) -> Self {
        Self {
            memory_map,
            next: 0,
        }
    }

    /// Get the [MemoryRegionType] of a frame
    pub fn get_frame_type(&self, frame: PhysFrame) -> Option<MemoryRegionType> {
        self.memory_map
            .iter()
            .find(|v| {
                let addr = frame.start_address().as_u64();

                v.start >= addr && addr < v.end
            })
            .map(|v| v.kind)
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionType::Usable);
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for GlobalFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;

        frame
    }
}
