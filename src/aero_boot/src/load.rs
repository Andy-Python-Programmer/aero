use aero_boot::FrameBufferInfo;

use x86_64::PhysAddr;

use uefi::{
    prelude::*,
    proto::media::{file::*, fs::SimpleFileSystem},
    table::boot::*,
};

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

pub fn load_and_switch_to_kernel(system_info: SystemInfo) {}
