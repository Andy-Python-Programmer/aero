#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, custom_test_frameworks)]
#![test_runner(crate::test_runner)]

extern crate rlibc;

use uefi::prelude::*;
use uefi::table::boot::MemoryType;

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize utils");

    // Reset console before doing anything else.
    system_table
        .stdout()
        .reset(false)
        .expect_success("Failed to reset output buffer");

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

    loop {}
}

#[cfg(test)]
pub(crate) fn test_runner(tests: &[&dyn Fn()]) {
    for test in tests {
        test();
    }
}
