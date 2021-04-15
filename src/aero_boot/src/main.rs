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

use core::fmt::Write;
use core::mem;
use core::slice;

use aero_boot::FrameBufferInfo;

use load::SystemInfo;
use logger::{LockedLogger, Logger};

use paging::BootFrameAllocator;

use uefi::{
    prelude::*,
    proto::console::gop::{GraphicsOutput, PixelFormat},
    table::boot::{MemoryDescriptor, MemoryType},
    table::cfg,
};

use x86_64::PhysAddr;

mod load;
mod logger;
mod paging;
mod unwind;

pub const KERNEL_ELF_PATH: &str = r"\efi\kernel\aero.elf";

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
            PixelFormat::Rgb => aero_boot::PixelFormat::BGR,
            PixelFormat::Bgr => aero_boot::PixelFormat::BGR,
            PixelFormat::Bitmask | PixelFormat::BltOnly => {
                panic!("Bitmask and BltOnly framebuffers are not supported")
            }
        },
        bytes_per_pixel: 4,
        stride: mode_info.stride(),
    };

    let global_logger = LockedLogger::new(Logger::new(slice, info));
    let locked_logger = logger::LOGGER.call_once(|| global_logger);

    log::set_logger(locked_logger).expect("Failed to set the global logger");
    log::set_max_level(log::LevelFilter::Info); // Log everything.

    (PhysAddr::new(framebuffer.as_mut_ptr() as u64), info)
}

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    writeln!(system_table.stdout(), "UEFI boot...").expect("Failed to write to stdout");

    let (framebuffer_address, framebuffer_info) = init_display(&system_table);
    log::info!("Using framebuffer at: {:#x}", framebuffer_address);

    let kernel_bytes = load::load_file(system_table.boot_services(), KERNEL_ELF_PATH);

    let mmap_storage = {
        let max_mmap_size =
            system_table.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();

        let ptr = system_table
            .boot_services()
            .allocate_pool(MemoryType::LOADER_DATA, max_mmap_size)?
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
        .map(|entry| PhysAddr::new(entry.address as u64));

    let system_info = SystemInfo {
        framebuffer_address,
        framebuffer_info,
        rsdp_address,
    };

    load::load_and_switch_to_kernel(frame_allocator, page_tables, kernel_bytes, system_info);
}
