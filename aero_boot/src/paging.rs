use uefi::table::boot::{MemoryDescriptor, MemoryType};

use x86_64::{
    registers::{
        control::{Cr0, Cr0Flags, Cr3, Cr3Flags},
        model_specific::{Efer, EferFlags},
    },
    structures::paging::*,
    PhysAddr, VirtAddr,
};

use xmas_elf::program::ProgramHeader;

pub struct BootFrameAllocator<I> {
    original: I,
    memory_map: I,
    current_descriptor: Option<MemoryDescriptor>,
    next_frame: PhysFrame,
}

impl<I> BootFrameAllocator<I>
where
    I: ExactSizeIterator<Item = MemoryDescriptor> + Clone,
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

    fn allocate_frame_from_descriptor(
        &mut self,
        descriptor: MemoryDescriptor,
    ) -> Option<PhysFrame> {
        let start_address = PhysAddr::new(descriptor.phys_start);
        let start_frame = PhysFrame::containing_address(start_address);

        let end_addr = start_address + (descriptor.page_count * Size4KiB::SIZE);
        let end_frame = PhysFrame::containing_address(end_addr - 1u64);

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

    pub fn max_physical_address(&self) -> PhysAddr {
        self.original
            .clone()
            .map(|r| PhysAddr::new(r.phys_start) + (r.page_count * Size4KiB::SIZE))
            .max()
            .unwrap()
    }
}

unsafe impl<I> FrameAllocator<Size4KiB> for BootFrameAllocator<I>
where
    I: ExactSizeIterator<Item = MemoryDescriptor> + Clone,
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

        while let Some(descriptor) = self.memory_map.next() {
            if descriptor.ty != MemoryType::CONVENTIONAL {
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

pub struct Level4Entries {
    entries: [bool; 512], // TODO: Use a bitmap class.
}

impl Level4Entries {
    pub fn new<'header>(segments: impl Iterator<Item = ProgramHeader<'header>>) -> Self {
        let mut this = Self {
            entries: [false; 512],
        };

        this.entries[0] = true;

        for segment in segments {
            let start_page: Page = Page::containing_address(VirtAddr::new(segment.virtual_addr()));
            let end_page: Page = Page::containing_address(VirtAddr::new(
                segment.virtual_addr() + segment.mem_size(),
            ));

            for p4_index in u64::from(start_page.p4_index())..=u64::from(end_page.p4_index()) {
                this.entries[p4_index as usize] = true;
            }
        }

        this
    }

    pub fn get_free_entry(&mut self) -> PageTableIndex {
        let (index, entry) = self
            .entries
            .iter_mut()
            .enumerate()
            .find(|(_, &mut entry)| entry == false)
            .unwrap();

        *entry = true;
        PageTableIndex::new(index as u16)
    }

    pub fn get_free_address(&mut self) -> VirtAddr {
        Page::from_page_table_indices_1gib(self.get_free_entry(), PageTableIndex::new(0))
            .start_address()
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
