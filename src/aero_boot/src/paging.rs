use aero_boot::MemoryRegionType;
use uefi::table::boot::MemoryDescriptor;

use x86_64::{structures::paging::*, PhysAddr};

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
        todo!()
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
