use core::mem::MaybeUninit;

use aero_boot::{MemoryRegion, MemoryRegionType};

use uefi::table::boot::{MemoryDescriptor, MemoryType};
use x86_64::VirtAddr;

use x86_64::{
    registers::{
        control::*,
        model_specific::{Efer, EferFlags},
    },
    structures::paging::*,
    PhysAddr,
};

pub trait BootMemoryRegion: Copy + core::fmt::Debug {
    /// Returns the physical start address of the region.
    fn start(&self) -> PhysAddr;

    /// Returns the size of the region in bytes.
    fn len(&self) -> u64;

    /// Returns the type of the region
    fn region_type(&self) -> MemoryRegionType;
}

impl<'a> BootMemoryRegion for MemoryDescriptor {
    fn start(&self) -> PhysAddr {
        PhysAddr::new(self.phys_start)
    }

    fn len(&self) -> u64 {
        self.page_count * Size4KiB::SIZE
    }

    fn region_type(&self) -> MemoryRegionType {
        match self.ty {
            MemoryType::CONVENTIONAL => MemoryRegionType::Usable,
            other => MemoryRegionType::UnknownUefi(other.0),
        }
    }
}

pub struct BootFrameAllocator<I, D> {
    original: I,
    memory_map: I,
    current_descriptor: Option<D>,
    next_frame: PhysFrame,
}

impl<I, D> BootFrameAllocator<I, D>
where
    I: ExactSizeIterator<Item = D> + Clone,
    I::Item: BootMemoryRegion,
{
    pub fn new(memory_map: I) -> Self {
        let start_frame = PhysFrame::containing_address(PhysAddr::new(0x1000));

        Self {
            original: memory_map.clone(),
            memory_map,
            current_descriptor: None,
            next_frame: start_frame,
        }
    }

    pub fn allocate_frame_from_descriptor(&mut self, descriptor: D) -> Option<PhysFrame> {
        let start_addr = descriptor.start();
        let start_frame = PhysFrame::containing_address(start_addr);
        let end_addr = start_addr + descriptor.len();
        let end_frame = PhysFrame::containing_address(end_addr - 1u64);

        // Set next_frame to start_frame if its smaller then next_frame.
        if self.next_frame < start_frame {
            self.next_frame = start_frame;
        }

        if self.next_frame < end_frame {
            let frame = self.next_frame;
            self.next_frame += 1;

            Some(frame)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.original.len()
    }

    pub fn max_physical_address(&self) -> PhysAddr {
        self.original
            .clone()
            .map(|r| r.start() + r.len())
            .max()
            .unwrap()
    }

    pub fn construct_memory_map(
        self,
        regions: &mut [MaybeUninit<MemoryRegion>],
    ) -> &mut [MemoryRegion] {
        let mut next_index = 0;

        for descriptor in self.original {
            let mut start = descriptor.start();
            let end = start + descriptor.len();
            let next_free = self.next_frame.start_address();
            let kind = match descriptor.region_type() {
                MemoryRegionType::Usable => {
                    if end <= next_free {
                        MemoryRegionType::Bootloader
                    } else if descriptor.start() >= next_free {
                        MemoryRegionType::Usable
                    } else {
                        // part of the region is used -> add it separately
                        let used_region = MemoryRegion {
                            start: descriptor.start().as_u64(),
                            end: next_free.as_u64(),
                            kind: MemoryRegionType::Bootloader,
                        };
                        Self::add_region(used_region, regions, &mut next_index)
                            .expect("Failed to add memory region");

                        // add unused part normally
                        start = next_free;
                        MemoryRegionType::Usable
                    }
                }
                MemoryRegionType::UnknownUefi(other) => {
                    use uefi::table::boot::MemoryType as M;
                    match M(other) {
                        M::LOADER_CODE
                        | M::LOADER_DATA
                        | M::BOOT_SERVICES_CODE
                        | M::BOOT_SERVICES_DATA
                        | M::RUNTIME_SERVICES_CODE
                        | M::RUNTIME_SERVICES_DATA => MemoryRegionType::Usable,
                        other => MemoryRegionType::UnknownUefi(other.0),
                    }
                }

                other => other,
            };

            let region = MemoryRegion {
                start: start.as_u64(),
                end: end.as_u64(),
                kind,
            };

            Self::add_region(region, regions, &mut next_index).unwrap();
        }

        let initialized = &mut regions[..next_index];
        unsafe { MaybeUninit::slice_assume_init_mut(initialized) }
    }

    fn add_region(
        region: MemoryRegion,
        regions: &mut [MaybeUninit<MemoryRegion>],
        next_index: &mut usize,
    ) -> Result<(), ()> {
        unsafe {
            regions
                .get_mut(*next_index)
                .ok_or(())?
                .as_mut_ptr()
                .write(region)
        };

        *next_index += 1;
        Ok(())
    }
}

unsafe impl<I, D> FrameAllocator<Size4KiB> for BootFrameAllocator<I, D>
where
    I: ExactSizeIterator<Item = D> + Clone,
    I::Item: BootMemoryRegion,
{
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if let Some(current_descriptor) = self.current_descriptor {
            match self.allocate_frame_from_descriptor(current_descriptor) {
                Some(frame) => return Some(frame),
                None => {
                    self.current_descriptor = None;
                }
            }
        }

        // Find next suitable descriptor
        while let Some(descriptor) = self.memory_map.next() {
            if descriptor.region_type() != MemoryRegionType::Usable {
                continue;
            }

            if let Some(frame) = self.allocate_frame_from_descriptor(descriptor) {
                self.current_descriptor = Some(descriptor);
                return Some(frame);
            }
        }

        None
    }
}

pub struct PageTables {
    pub boot_page_table: OffsetPageTable<'static>,
    pub kernel_page_table: OffsetPageTable<'static>,
    pub kernel_level_4_frame: PhysFrame,
}

pub fn init(frame_allocator: &mut impl FrameAllocator<Size4KiB>) -> PageTables {
    let physical_offset = VirtAddr::new(0x00);

    let old_table = {
        let frame = Cr3::read().0;
        let ptr: *const PageTable = (physical_offset + frame.start_address().as_u64()).as_ptr();

        unsafe { &*ptr }
    };

    let new_frame = frame_allocator.allocate_frame().unwrap();

    let new_table: &mut PageTable = {
        let ptr: *mut PageTable =
            (physical_offset + new_frame.start_address().as_u64()).as_mut_ptr();

        unsafe {
            ptr.write(PageTable::new());

            &mut *ptr
        }
    };

    // Copy the first entry (we don't need to access more than 512 GiB; also, some UEFI
    // implementations seem to create an level 4 table entry 0 in all slots)
    new_table[0] = old_table[0].clone();

    let boot_page_table = unsafe {
        Cr3::write(new_frame, Cr3Flags::empty());
        OffsetPageTable::new(&mut *new_table, physical_offset)
    };

    let (kernel_page_table, kernel_level_4_frame) = {
        let frame: PhysFrame = frame_allocator.allocate_frame().expect("no unused frames");
        log::info!("Created a new page table for the kernel at: {:#?}", &frame);

        let addr = physical_offset + frame.start_address().as_u64();

        // Initialize a new page table.
        let ptr = addr.as_mut_ptr();
        unsafe { *ptr = PageTable::new() };

        let level_4_table = unsafe { &mut *ptr };
        (
            unsafe { OffsetPageTable::new(level_4_table, physical_offset) },
            frame,
        )
    };

    PageTables {
        boot_page_table,
        kernel_page_table,
        kernel_level_4_frame,
    }
}

pub fn enable_no_execute() {
    unsafe { Efer::update(|efer| *efer |= EferFlags::NO_EXECUTE_ENABLE) }
}

pub fn enable_protection() {
    unsafe { Cr0::update(|cr0| *cr0 |= Cr0Flags::WRITE_PROTECT) };
}
