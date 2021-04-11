#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, custom_test_frameworks)]
#![test_runner(aero_boot::test_runner)]

extern crate rlibc;

use core::mem;

use aero_boot::{BootInfo, FrameBufferInfo};

use paging::{BootFrameAllocator, Level4Entries, PageTables};
use x86_64::{registers, structures::paging::*, PhysAddr, VirtAddr};

use uefi::{
    prelude::*,
    proto::{
        console::gop::GraphicsOutput,
        media::{file::*, fs::SimpleFileSystem},
    },
    table::boot::*,
};

use xmas_elf::{
    header,
    program::{self, ProgramHeader, Type},
    ElfFile,
};

const KERNEL_ELF_PATH: &str = r"\efi\kernel\aero";
const SIZE_4_KIB_ZERO_ARRAY: Size4KiBPageArray = [0; Size4KiB::SIZE as usize / 8];

type Size4KiBPageArray = [u64; Size4KiB::SIZE as usize / 8];

mod paging;

struct KernelInfo {
    entry_point: VirtAddr,
    stack_top: VirtAddr,
}

fn initialize_gop(system_table: &SystemTable<Boot>) -> FrameBufferInfo {
    log::info!("Initializing GOP");

    let gop = system_table
        .boot_services()
        .locate_protocol::<GraphicsOutput>()
        .expect_success("Failed to locate GOP");

    let gop = unsafe { &mut *gop.get() };

    let mode_info = gop.current_mode_info();
    let (width, height) = mode_info.resolution();

    FrameBufferInfo {
        horizontal_resolution: width,
        vertical_resolution: height,
        stride: mode_info.stride(),
    }
}

fn load_file(boot_services: &BootServices, path: &str) -> &'static [u8] {
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
    frame_allocator: &mut BootFrameAllocator,
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
    system_table: &SystemTable<Boot>,
    frame_allocator: &mut BootFrameAllocator,
    kernel_page_table: &mut OffsetPageTable,
) -> KernelInfo {
    log::info!("Loading kernel");

    let kernel_bin = load_file(system_table.boot_services(), KERNEL_ELF_PATH);
    let kernel_elf = ElfFile::new(&kernel_bin).expect("Found corrupt kernel ELF file");
    let kernel_offset = PhysAddr::new(&kernel_bin[0] as *const u8 as u64);

    assert!(kernel_offset.is_aligned(Size4KiB::SIZE));

    header::sanity_check(&kernel_elf).expect("Failed the sanity check for the kernel");

    log::info!(
        "Found kernel entry point at: {:#06x}",
        kernel_elf.header.pt2.entry_point()
    );

    for header in kernel_elf.program_iter() {
        program::sanity_check(header, &kernel_elf).expect("Failed header sanity check");

        match header.get_type().expect("Unable to get the header type") {
            Type::Load => map_segment(&header, kernel_offset, frame_allocator, kernel_page_table),
            _ => (),
        }
    }

    // Create stack for the kernel.
    let mut used_entries = Level4Entries::new(kernel_elf.program_iter());

    let stack_start_address = used_entries.get_free_address();
    let stack_end_address = stack_start_address + 20 * Size4KiB::SIZE;

    let stack_start: Page = Page::containing_address(stack_start_address);
    let stack_end: Page = Page::containing_address(stack_end_address - 1u64);

    for page in Page::range_inclusive(stack_start, stack_end) {
        let frame = frame_allocator.allocate_frame().unwrap();

        unsafe {
            kernel_page_table
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

    log::info!("Mapping physical memory");

    KernelInfo {
        entry_point: VirtAddr::new(kernel_elf.header.pt2.entry_point()),
        stack_top: stack_end.start_address(),
    }
}

fn switch_to_kernel(
    kernel_info: KernelInfo,
    boot_info: BootInfo,
    frame_allocator: &mut BootFrameAllocator,
    page_tables: &mut PageTables,
) -> ! {
    paging::enable_no_execute();
    paging::enable_protection();

    let current_address = PhysAddr::new(registers::read_rip().as_u64());
    let current_frame: PhysFrame = PhysFrame::containing_address(current_address);

    for frame in PhysFrame::range_inclusive(current_frame, current_frame + 1) {
        unsafe {
            page_tables
                .kernel_page_table
                .identity_map(frame, PageTableFlags::PRESENT, frame_allocator)
                .unwrap()
                .flush();
        }
    }

    // unsafe {
    // let kernel_level_4_start = page_tables.kernel_level_4_frame.start_address().as_u64();
    // let stack_top = kernel_info.stack_top.as_u64();
    // let entry_point = kernel_info.entry_point.as_u64();

    // asm!("mov cr3, {}", in(reg) kernel_level_4_start);
    // asm!("mov rsp, {}", in(reg) stack_top);
    // asm!("push 0");
    // asm!("jmp {}", in(reg) entry_point, in("rdi") &boot_info as *const _ as usize);
    // }

    loop {}
}

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize utils");

    // Reset console before doing anything else.
    system_table
        .stdout()
        .reset(false)
        .expect_success("Failed to reset output buffer");

    // Set up the boot frame allocator used at boot stage.
    // Note: Boot frame allocator is dropped after exiting boot services.
    let mut boot_frame_allocator = BootFrameAllocator::new(system_table.boot_services());
    let mut page_tables = paging::init(&mut boot_frame_allocator);

    let frame_buffer_info = initialize_gop(&system_table);
    let kernel_main_address = load_kernel(
        &system_table,
        &mut boot_frame_allocator,
        &mut page_tables.kernel_page_table,
    );

    log::info!("Exiting boot services");

    // let buffer_size = system_table.boot_services().memory_map_size() * 2;
    // let buffer_ptr = system_table
    //     .boot_services()
    //     .allocate_pool(MemoryType::LOADER_DATA, buffer_size)
    //     .expect_success("Failed to allocate pool");

    // let mmap_buffer = unsafe { core::slice::from_raw_parts_mut(buffer_ptr, buffer_size) };

    // drop(boot_frame_allocator);

    // let (_, _) = system_table
    //     .exit_boot_services(image, mmap_buffer)
    //     .expect_success("Failed to exit boot services.");

    let boot_info = BootInfo { frame_buffer_info };

    switch_to_kernel(
        kernel_main_address,
        boot_info,
        &mut boot_frame_allocator,
        &mut page_tables,
    );
}

pub fn align_up(address: u64, align: u64) -> u64 {
    let align_mask = align - 1;

    if address & align_mask == 0 {
        address // Address is already aligned.
    } else {
        (address | align_mask) + 1
    }
}
