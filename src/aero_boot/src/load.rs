use aero_boot::FrameBufferInfo;

use x86_64::{align_up, structures::paging::*, PhysAddr, VirtAddr};

use uefi::{
    prelude::*,
    proto::media::{file::*, fs::SimpleFileSystem},
    table::boot::*,
};

use xmas_elf::{
    header,
    program::{self, ProgramHeader, Type},
    ElfFile,
};

use crate::paging::{self, BootFrameAllocator};
use crate::paging::{BootMemoryRegion, PageTables};

const SIZE_4_KIB_ZERO_ARRAY: Size4KiBPageArray = [0; Size4KiB::SIZE as usize / 8];
type Size4KiBPageArray = [u64; Size4KiB::SIZE as usize / 8];

/// Required system information that should be queried from the BIOS or UEFI firmware.
#[derive(Debug, Copy, Clone)]
pub struct SystemInfo {
    /// Start address of the pixel-based framebuffer.
    pub framebuffer_address: PhysAddr,
    /// Information about the framebuffer, including layout and pixel format.
    pub framebuffer_info: FrameBufferInfo,
    /// Address of the _Root System Description Pointer_ structure of the ACPI standard.
    pub rsdp_address: Option<PhysAddr>,
}

/// Contains the addresses of all memory mappings set up by [`set_up_mappings`].
pub struct Mappings {
    pub entry_point: VirtAddr,
    pub stack_end: Page,
    pub used_entries: Level4Entries,
    pub framebuffer: VirtAddr,
    pub physical_memory_offset: VirtAddr,
}

/// Keeps track of used entries in a level 4 page table.
///
/// Useful for determining a free virtual memory block, e.g. for mapping additional data.
#[derive(Debug)]
pub struct Level4Entries {
    entries: [bool; 512], // TODO: Use bitmap class to speed up process
}

impl Level4Entries {
    fn new<'a>(segments: impl Iterator<Item = ProgramHeader<'a>>) -> Self {
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

    fn get_free_entry(&mut self) -> PageTableIndex {
        let (idx, entry) = self
            .entries
            .iter_mut()
            .enumerate()
            .find(|(_, &mut entry)| entry == false)
            .expect("No usable level 4 entries found");

        *entry = true;
        PageTableIndex::new(idx as u16)
    }

    fn get_free_address(&mut self) -> VirtAddr {
        Page::from_page_table_indices_1gib(self.get_free_entry(), PageTableIndex::new(0))
            .start_address()
    }
}

pub fn load_file(boot_services: &BootServices, path: &str) -> &'static [u8] {
    let mut info_buffer = [0u8; 0x100];

    let file_system = unsafe {
        &mut *boot_services
            .locate_protocol::<SimpleFileSystem>()
            .expect_success("Failed to locate file system")
            .get()
    };

    let mut root = file_system
        .open_volume()
        .expect_success("Failed to open volumes");

    let volume_label = file_system
        .open_volume()
        .expect_success("Failed to open volume")
        .get_info::<FileSystemVolumeLabel>(&mut info_buffer)
        .expect_success("Failed to open volumes")
        .volume_label();

    log::info!("Volume label: {}", volume_label);

    let file_handle = root
        .open(path, FileMode::Read, FileAttribute::empty())
        .expect_success("Failed to open file");

    let mut file_handle = unsafe { RegularFile::new(file_handle) };

    log::info!("Loading {} into memory", path);

    let info = file_handle
        .get_info::<FileInfo>(&mut info_buffer)
        .expect_success("Failed to get file info");

    let pages = info.file_size() as usize / 0x1000 + 1;
    let mem_start = boot_services
        .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, pages)
        .expect_success("Failed to allocate pages");

    let buffer = unsafe { core::slice::from_raw_parts_mut(mem_start as *mut u8, pages * 0x1000) };
    let length = file_handle
        .read(buffer)
        .expect_success("Failed to read file");

    buffer[..length].as_ref()
}

fn map_segment(
    segment: &ProgramHeader,
    kernel_offset: PhysAddr,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    page_table: &mut OffsetPageTable,
) {
    let physical_address = kernel_offset + segment.offset();
    let start_frame: PhysFrame = PhysFrame::containing_address(physical_address);
    let end_frame: PhysFrame =
        PhysFrame::containing_address(physical_address + segment.file_size() - 1u64);

    let virtual_start = VirtAddr::new(segment.virtual_addr());
    let start_page: Page = Page::containing_address(virtual_start);

    let flags = segment.flags();
    let mut page_table_flags = PageTableFlags::PRESENT;

    if !flags.is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE
    }

    if flags.is_write() {
        page_table_flags |= PageTableFlags::WRITABLE
    }

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let offset = frame - start_frame;
        let page = start_page + offset;

        unsafe {
            // We operate on an inactive page table, so there's no need to flush anything.

            page_table
                .map_to(page, frame, page_table_flags, frame_allocator)
                .unwrap()
                .ignore();
        }
    }

    // Handle the `.bss` sectiton.
    if segment.mem_size() > segment.file_size() {
        let zero_start = virtual_start + segment.file_size();
        let zero_end = virtual_start + segment.mem_size();

        if zero_start.as_u64() & 0xfff != 0 {
            let orignal_frame: PhysFrame =
                PhysFrame::containing_address(physical_address + segment.file_size() - 1u64);

            let new_frame = frame_allocator.allocate_frame().unwrap();

            let new_frame_ptr = new_frame.start_address().as_u64() as *mut Size4KiBPageArray;
            unsafe { new_frame_ptr.write(SIZE_4_KIB_ZERO_ARRAY) };

            drop(new_frame_ptr);

            // Copy the data from the orignal frame to the new frame.

            let orig_bytes_ptr = orignal_frame.start_address().as_u64() as *mut u8;
            let new_bytes_ptr = new_frame.start_address().as_u64() as *mut u8;

            for offset in 0..((zero_start.as_u64() & 0xfff) as isize) {
                unsafe {
                    let orig_byte = orig_bytes_ptr.offset(offset).read();
                    new_bytes_ptr.offset(offset).write(orig_byte);
                }
            }

            let last_page = Page::containing_address(virtual_start + segment.file_size() - 1u64);

            unsafe {
                page_table.unmap(last_page).unwrap().1.ignore();
                page_table
                    .map_to(last_page, new_frame, page_table_flags, frame_allocator)
                    .unwrap()
                    .ignore();
            }
        }

        let start_page: Page =
            Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page = Page::containing_address(zero_end);

        // Map additional frames for the `.bss` section.
        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator.allocate_frame().unwrap();

            let frame_ptr = frame.start_address().as_u64() as *mut Size4KiBPageArray;
            unsafe { frame_ptr.write(SIZE_4_KIB_ZERO_ARRAY) };

            drop(frame_ptr);

            unsafe {
                page_table
                    .map_to(page, frame, page_table_flags, frame_allocator)
                    .unwrap()
                    .ignore();
            }
        }
    }
}

fn load_kernel(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    page_tables: &mut PageTables,
    kernel_bytes: &[u8],
) -> (u64, Level4Entries) {
    log::info!("Loading kernel");

    paging::enable_no_execute();
    paging::enable_protection();

    let kernel_elf = ElfFile::new(&kernel_bytes).expect("Found corrupt kernel ELF file");
    let kernel_offset = PhysAddr::new(&kernel_bytes[0] as *const u8 as u64);

    assert!(kernel_offset.is_aligned(Size4KiB::SIZE));

    header::sanity_check(&kernel_elf).expect("Failed the sanity check for the kernel");

    let entry_point = kernel_elf.header.pt2.entry_point();
    log::info!("Found kernel entry point at: {:#x}", entry_point);

    for header in kernel_elf.program_iter() {
        program::sanity_check(header, &kernel_elf).expect("Failed header sanity check");

        match header.get_type().expect("Unable to get the header type") {
            Type::Load => map_segment(
                &header,
                kernel_offset,
                frame_allocator,
                &mut page_tables.kernel_page_table,
            ),
            _ => (),
        }
    }

    let used_entries = Level4Entries::new(kernel_elf.program_iter());

    (entry_point, used_entries)
}

fn set_up_mappings<I, D>(
    frame_allocator: &mut BootFrameAllocator<I, D>,
    page_tables: &mut PageTables,
    system_info: SystemInfo,
    kernel_entry: u64,
    mut used_entries: Level4Entries,
) -> Mappings
where
    I: ExactSizeIterator<Item = D> + Clone,
    D: BootMemoryRegion,
{
    let entry_point = VirtAddr::new(kernel_entry);

    // Create a stack for the kernel.
    log::info!("Creating a stack for the kernel");

    let stack_start_addr = used_entries.get_free_address();
    let stack_start: Page = Page::containing_address(stack_start_addr);

    let stack_end_addr = stack_start_addr + (20 * Size4KiB::SIZE);
    let stack_end: Page = Page::containing_address(stack_end_addr - 1u64);

    for page in Page::range_inclusive(stack_start, stack_end) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("frame allocation failed when mapping a kernel stack");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            page_tables
                .kernel_page_table
                .map_to(page, frame, flags, frame_allocator)
                .unwrap()
                .flush();
        }
    }

    // Map the framebuffer.
    log::info!("Mapping framebuffer");

    let framebuffer_start_frame: PhysFrame =
        PhysFrame::containing_address(system_info.framebuffer_address);
    let framebuffer_end_frame = PhysFrame::containing_address(
        system_info.framebuffer_address + system_info.framebuffer_info.byte_len - 1u64,
    );

    let framebuffer_start_page = Page::containing_address(used_entries.get_free_address());

    for (i, frame) in
        PhysFrame::range_inclusive(framebuffer_start_frame, framebuffer_end_frame).enumerate()
    {
        let page = framebuffer_start_page + i as u64;

        unsafe {
            page_tables
                .kernel_page_table
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .flush();
        }
    }

    let framebuffer = framebuffer_start_page.start_address();

    // Get the physical memory offset.
    let physical_memory_offset = used_entries.get_free_address();

    let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
    let max_physical = frame_allocator.max_physical_address();

    let end_frame: PhysFrame<Size2MiB> = PhysFrame::containing_address(max_physical - 1u64);

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let page =
            Page::containing_address(physical_memory_offset + frame.start_address().as_u64());

        unsafe {
            page_tables
                .kernel_page_table
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .ignore();
        }
    }

    Mappings {
        entry_point,
        stack_end,
        used_entries,
        framebuffer,
        physical_memory_offset,
    }
}

pub fn load_and_switch_to_kernel<I, D>(
    frame_allocator: &mut BootFrameAllocator<I, D>,
    page_tables: &mut PageTables,
    kernel_bytes: &[u8],
    system_info: SystemInfo,
) where
    I: ExactSizeIterator<Item = D> + Clone,
    D: BootMemoryRegion,
{
    let (kernel_entry, used_entries) = load_kernel(frame_allocator, page_tables, kernel_bytes);

    let mappings = set_up_mappings(
        frame_allocator,
        page_tables,
        system_info,
        kernel_entry,
        used_entries,
    );
}
