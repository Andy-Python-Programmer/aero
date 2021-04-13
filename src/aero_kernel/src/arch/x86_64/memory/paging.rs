use bootloader::{boot_info::*, BootInfo};
use x86_64::{
    registers::control::Cr3,
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, OffsetPageTable, Page, PageTable,
        PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

pub struct UnmapGuard {
    pub page: Page<Size4KiB>,
}

impl UnmapGuard {
    #[inline]
    fn new(page: Page<Size4KiB>) -> Self {
        Self { page }
    }
}

pub struct GlobalAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl GlobalAllocator {
    /// Create a new global frame allocator from the memory map provided by the bootloader.
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        Self {
            memory_map,
            next: 0,
        }
    }

    /// Get the [MemoryRegionKind] of a frame
    pub fn get_frame_type(&self, frame: PhysFrame) -> Option<MemoryRegionKind> {
        self.memory_map
            .into_iter()
            .find(|v| {
                let addr = frame.start_address().as_u64();

                v.start >= addr && addr < v.end
            })
            .map(|v| v.kind)
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for GlobalAllocator {
    #[track_caller]
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;

        frame
    }
}

unsafe impl Sync for GlobalAllocator {}
unsafe impl Send for GlobalAllocator {}

/// Initialize paging.
pub fn init(boot_info: &'static BootInfo) -> (OffsetPageTable, GlobalAllocator) {
    let physical_memory_offset =
        VirtAddr::new(boot_info.physical_memory_offset.into_option().unwrap());

    unsafe {
        let active_level_4 = active_level_4_table(physical_memory_offset);

        let offset_table = OffsetPageTable::new(active_level_4, physical_memory_offset);
        let frame_allocator = GlobalAllocator::init(&boot_info.memory_regions);

        (offset_table, frame_allocator)
    }
}

/// Get a mutable reference to the active level 4 page table.
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let physical = level_4_table_frame.start_address();
    let virtual_address = physical_memory_offset + physical.as_u64();
    let page_table_ptr: *mut PageTable = virtual_address.as_mut_ptr();

    &mut *page_table_ptr
}

/// Identity maps a frame for a memory mapped device.
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
        MemoryRegionKind::UnknownBios(_) | MemoryRegionKind::UnknownUefi(_) => {
            PageTableFlags::WRITABLE
        }
        _ => panic!(
            "Tried to memory map a device on a {:?} frame {:#X}",
            frame_type,
            frame.start_address()
        ),
    };

    let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64()));

    offset_table
        .identity_map(
            frame,
            PageTableFlags::PRESENT
                | PageTableFlags::NO_CACHE
                | PageTableFlags::WRITE_THROUGH
                | extra_flags,
            frame_allocator,
        )?
        .flush();

    Ok(UnmapGuard::new(page))
}
