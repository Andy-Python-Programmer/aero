#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, custom_test_frameworks)]
#![test_runner(crate::test_runner)]

extern crate rlibc;

use aero_boot::FrameBufferInfo;
use uefi::{
    prelude::*,
    proto::{
        console::gop::GraphicsOutput,
        media::{
            file::{File, FileAttribute, FileMode},
            fs::SimpleFileSystem,
        },
    },
};
use uefi::{proto::media::file::FileType, table::boot::MemoryType};
use xmas_elf::{header, ElfFile};

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

pub fn load_file(
    handle: Handle,
    boot_services: &BootServices,
    path: &str,
) -> Result<&'static mut [u8], Status> {
    let loaded_image = unsafe {
        match boot_services.handle_protocol::<uefi::proto::loaded_image::LoadedImage>(handle) {
            Ok(val) => val.unwrap().get().as_ref().unwrap(),
            Err(_) => return Err(Status::LOAD_ERROR),
        }
    };

    let file_system = unsafe {
        match boot_services.handle_protocol::<SimpleFileSystem>(loaded_image.device()) {
            Ok(val) => val.unwrap().get().as_mut().unwrap(),
            Err(_) => return Err(Status::LOAD_ERROR),
        }
    };

    let mut root = match file_system.open_volume() {
        Ok(val) => val.unwrap(),
        Err(err) => return Err(err.status()),
    };

    let path_pool = match boot_services.allocate_pool(MemoryType::LOADER_DATA, path.len()) {
        Ok(val) => val.unwrap(),
        Err(err) => return Err(err.status()),
    };

    for (index, c) in path.chars().enumerate() {
        unsafe {
            path_pool.add(index).write(c as u8);
        }
    }

    let path = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_pool, path.len()))
    };

    let handle = match root
        .handle()
        .open(path, FileMode::Read, FileAttribute::empty())
    {
        Ok(handle) => handle.unwrap(),
        Err(err) => {
            boot_services.free_pool(path_pool).unwrap().unwrap();
            return Err(err.status());
        }
    };

    boot_services.free_pool(path_pool).unwrap().unwrap();

    let mut file = match handle.into_type().unwrap().unwrap() {
        FileType::Regular(file) => file,
        FileType::Dir(_) => return Err(Status::ACCESS_DENIED),
    };

    match file.set_position(u64::MAX) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };

    let file_size = match file.get_position() {
        Ok(val) => val.unwrap() as usize,
        Err(err) => return Err(err.status()),
    };

    match file.set_position(0) {
        Ok(_) => (),
        Err(err) => return Err(err.status()),
    };

    let pool = match boot_services.allocate_pool(MemoryType::LOADER_DATA, file_size) {
        Ok(val) => val.unwrap(),
        Err(err) => return Err(err.status()),
    };

    let buffer = unsafe { core::slice::from_raw_parts_mut(pool, file_size) };

    if let Err(err) = file.read(buffer) {
        boot_services.free_pool(pool).unwrap().unwrap();

        return Err(err.status());
    }

    Ok(buffer)
}

fn load_kernel(image: Handle, system_table: &SystemTable<Boot>) {
    log::info!("Loading kernel");

    let kernel_bin = load_file(image, system_table.boot_services(), "\\efi\\kernel\\aero")
        .expect("Failed to load the kernel");

    let kernel_elf = ElfFile::new(&kernel_bin).expect("Found corrupt kernel ELF file");

    header::sanity_check(&kernel_elf).expect("Failed the sanity check for the kernel");
    log::info!(
        "Jumping to kernel entry point at: {:#06x}",
        kernel_elf.header.pt2.entry_point()
    )
}

fn switch_to_kernel() -> ! {
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

    initialize_gop(&system_table);
    load_kernel(image, &system_table);

    log::info!("Exiting boot services");

    let buffer_size = system_table.boot_services().memory_map_size() * 2;
    let buffer_ptr = system_table
        .boot_services()
        .allocate_pool(MemoryType::LOADER_DATA, buffer_size)
        .expect_success("Failed to allocate pool");

    let mmap_buffer = unsafe { core::slice::from_raw_parts_mut(buffer_ptr, buffer_size) };

    system_table
        .exit_boot_services(image, mmap_buffer)
        .expect_success("Failed to exit boot services.");

    switch_to_kernel();
}

#[cfg(test)]
pub(crate) fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}