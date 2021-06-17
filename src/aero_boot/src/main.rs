// This crate contains some code borrowed from https://github.com/rust-osdev/bootloader/

#![no_std]
#![no_main]
#![feature(
    asm,
    abi_efiapi,
    custom_test_frameworks,
    maybe_uninit_extra,
    maybe_uninit_slice
)]
#![test_runner(aero_boot::test_runner)]

extern crate rlibc;

use core::mem;
use core::slice;

use aero_gfx::{debug::rendy::DebugRendy, FrameBufferInfo};

use load::SystemInfo;
use logger::LockedLogger;

use paging::BootFrameAllocator;

use uefi::{
    prelude::*,
    proto::{
        console::gop::{GraphicsOutput, PixelFormat},
        media::{
            file::{File, FileSystemVolumeLabel},
            fs::SimpleFileSystem,
        },
    },
    table::boot::{MemoryDescriptor, MemoryType},
    table::cfg,
};

use x86_64::PhysAddr;

use crate::load::BootFileSystem;

mod load;
mod logger;
mod paging;
mod unwind;

const AERO_KERNEL_ELF_PATH: &str = r"aero_kernel.elf";
const AERO_PHYSICAL_OFFSET: u64 = 0xFFFF800000000000;
const AERO_STACK_ADDRESS: u64 = 0x10000000;

fn init_display(system_table: &SystemTable<Boot>) -> (PhysAddr, FrameBufferInfo) {
    let gop = system_table
        .boot_services()
        .locate_protocol::<GraphicsOutput>()
        .expect_success("Failed to locate GOP");

    let gop = unsafe { &mut *gop.get() };

    let mode_info = gop.current_mode_info();
    let mut framebuffer = gop.frame_buffer();

    let slice = unsafe { slice::from_raw_parts_mut(framebuffer.as_mut_ptr(), framebuffer.size()) };

    let info = FrameBufferInfo {
        byte_len: framebuffer.size(),
        horizontal_resolution: mode_info.resolution().0,
        vertical_resolution: mode_info.resolution().1,
        pixel_format: match mode_info.pixel_format() {
            PixelFormat::Rgb => aero_gfx::PixelFormat::BGR,
            PixelFormat::Bgr => aero_gfx::PixelFormat::BGR,
            PixelFormat::Bitmask | PixelFormat::BltOnly => {
                panic!("Bitmask and BltOnly framebuffers are not supported")
            }
        },
        bytes_per_pixel: 4,
        stride: mode_info.stride(),
    };

    let global_logger = LockedLogger::new(DebugRendy::new(slice, info));
    let locked_logger = logger::LOGGER.call_once(|| global_logger);

    log::set_logger(locked_logger).expect("Failed to set the global logger");
    log::set_max_level(log::LevelFilter::Info); // Log everything.

    (PhysAddr::new(framebuffer.as_mut_ptr() as u64), info)
}

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    let (framebuffer_address, framebuffer_info) = init_display(&system_table);
    log::info!("Using framebuffer at: {:#x}", framebuffer_address);

    let mut info_buffer = [0u8; 0x100];

    let file_system = unsafe {
        &mut *system_table
            .boot_services()
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

    let mut boot_filesystem = BootFileSystem {
        info_buffer: &mut info_buffer,
        root: &mut root,
    };

    let kernel_bytes = load::load_file(
        &mut boot_filesystem,
        system_table.boot_services(),
        AERO_KERNEL_ELF_PATH,
    );

    let mmap_storage = {
        let max_mmap_size =
            system_table.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();

        let ptr = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, max_mmap_size)
            .unwrap()
            .log();

        unsafe { slice::from_raw_parts_mut(ptr, max_mmap_size) }
    };

    log::info!("Exiting boot services");

    let (system_table, memory_map) = system_table
        .exit_boot_services(image, mmap_storage)
        .expect_success("Failed to exit boot services");

    let mut frame_allocator = BootFrameAllocator::new(memory_map.copied());
    let page_tables = paging::init(&mut frame_allocator);

    let mut config_entries = system_table.config_table().iter();

    let rsdp_address = config_entries
        .find(|entry| matches!(entry.guid, cfg::ACPI_GUID | cfg::ACPI2_GUID))
        .map(|entry| PhysAddr::new(entry.address as u64))
        .expect("Aero requires ACPI compatible system");

    let system_info = SystemInfo {
        framebuffer_address,
        framebuffer_info,
        rsdp_address,
    };

    load::load_and_switch_to_kernel(frame_allocator, page_tables, kernel_bytes, system_info);
}
