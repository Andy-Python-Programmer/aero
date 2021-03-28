use bootloader::{
    bootinfo::{MemoryMap, MemoryRegionType},
    BootInfo,
};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTable,
        PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

const BITS_PER_FRAME: u64 = 0x1000 * 8;
const BITMAP_START: u64 = 0x666666660000;

pub struct UnmapGuard {
    pub page: Page<Size4KiB>,
    pub unmap_frame: bool,
}

pub struct BootAllocator {
    pub memory_map: &'static MemoryMap,
    pub next: usize,
    pub used: [u64; 64],
}

impl BootAllocator {
    pub fn allocate_bitmap_frame(&mut self, mapper: &mut impl Mapper<Size4KiB>, addr: u64) {
        let frame = self.allocate_frame().expect("Failed to allocate frame");

        unsafe {
            mapper
                .map_to(
                    Page::from_start_address_unchecked(VirtAddr::new(addr)),
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    self,
                )
                .expect("Failed to map")
                .flush()
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = (PhysFrame, usize)> {
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .enumerate()
            .filter(|(_, r)| r.region_type == MemoryRegionType::Usable);

        let addr_ranges =
            usable_regions.map(|(i, r)| (i, r.range.start_addr()..r.range.end_addr()));

        let frame_addresses =
            addr_ranges.flat_map(|(i, r)| r.step_by(4096).zip(core::iter::repeat(i)));

        frame_addresses.map(|(addr, i)| (PhysFrame::containing_address(PhysAddr::new(addr)), i))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let (frame, block) = self.usable_frames().nth(self.next)?;

        self.next += 1;
        self.used[block] += 1;

        Some(frame)
    }
}

pub struct GlobalAllocator<'a> {
    memory_map: &'static MemoryMap,
    next: u64,
    bitmap: &'a mut [u32],
}

impl<'a> GlobalAllocator<'a> {
    pub unsafe fn init(memory_map: &'static MemoryMap, mapper: &mut impl Mapper<Size4KiB>) -> Self {
        let end_frame = memory_map.last().unwrap().range.end_frame_number;
        let bitmap_frames = (end_frame + BITS_PER_FRAME as u64 - 1) / BITS_PER_FRAME as u64;

        let mut bootstrap = BootAllocator {
            memory_map,
            next: 0,
            used: [0; 64],
        };

        for i in 0..bitmap_frames {
            bootstrap.allocate_bitmap_frame(mapper, BITMAP_START + i * 0x1000);
        }

        let bitmap =
            core::slice::from_raw_parts_mut(BITMAP_START as *mut _, end_frame as usize + 1);

        let mut this = Self {
            memory_map,
            next: 0,
            bitmap,
        };

        for (block, size) in bootstrap.used.iter().enumerate().filter(|(_, s)| **s != 0) {
            let start = memory_map[block].range.start_frame_number;

            for i in start..(start + size) {
                this.mark_used(i)
            }
        }

        for region in bootstrap.memory_map.into_iter() {
            if let MemoryRegionType::Usable
            | MemoryRegionType::Reserved
            | MemoryRegionType::AcpiReclaimable
            | MemoryRegionType::FrameZero = region.region_type
            {
                continue;
            }

            let start = region.range.start_frame_number;
            let end = region.range.end_frame_number;

            for i in start..end {
                this.mark_used(i)
            }
        }

        this
    }

    /// Get the `MemoryRegionType` of a frame
    pub fn get_frame_type(&self, frame: PhysFrame) -> Option<MemoryRegionType> {
        self.memory_map
            .into_iter()
            .find(|v| {
                let addr = frame.start_address().as_u64();

                v.range.start_addr() >= addr && addr < v.range.end_addr()
            })
            .map(|v| v.region_type)
    }

    /// Check if the frame is already in use
    pub fn frame_in_use(&self, frame: PhysFrame<Size4KiB>) -> bool {
        self.is_used(frame.start_address().as_u64() / 0x1000)
    }

    /// Check if the frame `idx` is used
    fn is_used(&self, idx: u64) -> bool {
        let (int, mask) = frame_idx_to_parts(idx);

        self.bitmap[int] & mask != 0
    }

    /// Set the frame `idx` as used
    fn mark_used(&mut self, idx: u64) {
        let (int, mask) = frame_idx_to_parts(idx);

        self.bitmap[int] |= mask;
    }

    fn usable_frames_iter(&self, memory_map: &'static MemoryMap) -> impl Iterator<Item = u64> {
        let regions = memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        usable_regions.flat_map(|r| r.range.start_frame_number..r.range.end_frame_number)
    }

    /// Retuns true and sets `self.next_usable` to the index of the next usable
    /// frame if ther's one available otherwise returns false
    fn recalculate_next_usable(&mut self) -> bool {
        let iter = self
            .usable_frames_iter(self.memory_map)
            .skip_while(|r| *r < self.next);

        // Try to find a frame that isn't used
        for i in iter {
            if !self.is_used(i) {
                self.next = i;
                return true;
            }
        }

        // There are no usable frames
        false
    }
}

unsafe impl<'a> FrameAllocator<Size4KiB> for GlobalAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        if self.recalculate_next_usable() {
            let i = self.next;

            self.mark_used(i);

            let addr = PhysAddr::new(i * 0x1000);
            let frame = unsafe { PhysFrame::from_start_address_unchecked(addr) };

            Some(frame)
        } else {
            None
        }
    }
}

/// Initialize paging.
pub fn init(boot_info: &'static BootInfo) -> (OffsetPageTable, GlobalAllocator) {
    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);

    let mut offset_table = unsafe { init_offset_page_table(physical_memory_offset) };
    let frame_allocator =
        unsafe { GlobalAllocator::init(&boot_info.memory_map, &mut offset_table) };

    (offset_table, frame_allocator)
}

/// Initialize a new offset page table.
unsafe fn init_offset_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);

    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Get a mutable reference to the active level 4 page table.
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let physical = level_4_table_frame.start_address();
    let virtual_address = physical_memory_offset + physical.as_u64();
    let page_table_ptr: *mut PageTable = virtual_address.as_mut_ptr();

    &mut *page_table_ptr
}

/// Identity maps a frame for a memory mapped device
#[track_caller]
pub unsafe fn memory_map_device(
    offset_table: &mut OffsetPageTable,
    frame_allocator: &mut GlobalAllocator,
    frame: PhysFrame,
) -> Result<UnmapGuard, MapToError<Size4KiB>> {
    let frame_type = frame_allocator
        .get_frame_type(frame)
        .ok_or(MapToError::FrameAllocationFailed)?;

    let extra_flags = match frame_type {
        MemoryRegionType::Reserved | MemoryRegionType::FrameZero => PageTableFlags::WRITABLE,
        MemoryRegionType::KernelStack => PageTableFlags::empty(),
        _ => panic!(
            "Tried to memory map a device on a {:?} frame {:#X}",
            frame_type,
            frame.start_address()
        ),
    };

    let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));

    let flusher = offset_table.identity_map(
        frame,
        PageTableFlags::PRESENT
            | PageTableFlags::NO_CACHE
            | PageTableFlags::WRITE_THROUGH
            | extra_flags,
        frame_allocator,
    )?;

    flusher.flush();

    Ok(UnmapGuard {
        page,
        unmap_frame: !frame_allocator.frame_in_use(frame),
    })
}

fn frame_idx_to_parts(idx: u64) -> (usize, u32) {
    let int = idx as usize / 32;
    let bit = idx as u32 % 32;

    (int, 1 << bit)
}
