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

use spin::Once;
use stivale_boot::v2::{StivaleMemoryMapEntryType, StivaleMemoryMapTag};

use super::mapper::*;
use super::page::*;

use super::addr::PhysAddr;

use crate::utils::sync::Mutex;

pub struct LockedFrameAllocator(Once<Mutex<GlobalFrameAllocator>>);

impl LockedFrameAllocator {
    /// Constructs a new uninitialized and locked version of the global frame
    /// allocator.
    pub(super) const fn new_uninit() -> Self {
        Self(Once::new())
    }

    /// Initializes the inner locked global frame allocator.
    pub(super) fn init(&self, memory_map: &'static StivaleMemoryMapTag) {
        self.0
            .call_once(|| Mutex::new(GlobalFrameAllocator::new(memory_map)));
    }

    #[allow(unused)]
    pub fn get_frame_type(&self, frame: PhysFrame) -> Option<StivaleMemoryMapEntryType> {
        if let Some(ref mut allocator) = self.0.get() {
            allocator.lock().get_frame_type(frame)
        } else {
            None
        }
    }

    fn allocate_frame_inner<S: PageSize>(&self) -> Option<PhysFrame<S>> {
        self.0
            .get()
            .map(|m| m.lock().allocate_frame_inner::<S>())
            .unwrap_or(None)
    }
}

unsafe impl FrameAllocator<Size4KiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.allocate_frame_inner::<Size4KiB>()
    }
}

unsafe impl FrameAllocator<Size2MiB> for LockedFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size2MiB>> {
        self.allocate_frame_inner::<Size2MiB>()
    }
}

pub struct GlobalFrameAllocator {
    memory_map: &'static StivaleMemoryMapTag,
    next: usize,
}

impl GlobalFrameAllocator {
    /// Create a new global frame allocator from the memory map provided by the bootloader.
    fn new(memory_map: &'static StivaleMemoryMapTag) -> Self {
        Self {
            memory_map,
            next: 1,
        }
    }

    /// Get the [MemoryRegionType] of a frame
    pub fn get_frame_type(&self, frame: PhysFrame) -> Option<StivaleMemoryMapEntryType> {
        self.memory_map
            .iter()
            .find(|v| {
                let addr = frame.start_address().as_u64();

                v.base >= addr && addr < v.end_address()
            })
            .map(|v| v.entry_type())
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames<S>(&self) -> impl Iterator<Item = PhysFrame<S>>
    where
        S: PageSize,
    {
        let regions = self.memory_map.iter();
        let usable_regions =
            regions.filter(|r| r.entry_type() == StivaleMemoryMapEntryType::Usable);
        let addr_ranges = usable_regions.map(|r| r.base..r.end_address());
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(S::SIZE as usize));

        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }

    fn allocate_frame_inner<S: PageSize>(&mut self) -> Option<PhysFrame<S>> {
        let frame = self.usable_frames::<S>().nth(self.next);
        self.next += 1;

        frame
    }
}

unsafe impl FrameAllocator<Size4KiB> for GlobalFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.allocate_frame_inner::<Size4KiB>()
    }
}

unsafe impl FrameAllocator<Size2MiB> for GlobalFrameAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size2MiB>> {
        self.allocate_frame_inner::<Size2MiB>()
    }
}
