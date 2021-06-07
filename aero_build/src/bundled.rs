/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use std::fs;
use std::io;

use std::error::Error;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use std::process::Command;

use std::convert::TryFrom;

use crate::{AeroChainloader, BUNDLED_DIR};

const PREBUILT_OVMF_URL: &str =
    "https://github.com/rust-osdev/ovmf-prebuilt/releases/latest/download/";

const LIMINE_GITHUB_URL: &str = "https://github.com/limine-bootloader/limine";
const LIMINE_RELEASE_BRANCH: &str = "latest-binary";

const OVMF_FILES: [&str; 3] = [
    "OVMF-pure-efi.fd",
    "OVMF_CODE-pure-efi.fd",
    "OVMF_VARS-pure-efi.fd",
];

/// Download the latest release of the OVMF prebuilt files from `https://github.com/rust-osdev/ovmf-prebuilt`
/// and save them in the bundled/ovmf directory.
///
/// **Note**: The existing OVMF files will be overwritten.
pub async fn update_ovmf() -> Result<(), Box<dyn Error>> {
    let ovmf_out_dir = Path::new(BUNDLED_DIR).join("ovmf");

    fs::create_dir_all(&ovmf_out_dir)?;

    for ovmf_file in OVMF_FILES.iter() {
        println!("INFO: Downloading {}", ovmf_file);

        let response = reqwest::get(format!("{}{}", PREBUILT_OVMF_URL, ovmf_file)).await?;
        let bytes = response.bytes().await?;

        let mut output = File::create(ovmf_out_dir.join(ovmf_file))?;
        output.write_all(bytes.as_ref())?;
    }

    Ok(())
}

/// Run [update_ovmf] if the OVMF files do not exist.
///
/// **Note**: To update the existing OVMF files run `cargo boot update`.
pub async fn download_ovmf_prebuilt() -> Result<(), Box<dyn Error>> {
    let ovmf_out_dir = Path::new(BUNDLED_DIR).join("ovmf");

    for ovmf_file in OVMF_FILES.iter() {
        if !ovmf_out_dir.join(ovmf_file).exists() {
            update_ovmf().await?;

            return Ok(());
        }
    }

    Ok(())
}

/// Download the latest release of the limine prebuilt files from `https://github.com/limine-bootloader/limine/releases/latest/download/`
/// and save them in the build/efi/boot directory.
///
/// **Note**: The existing prebuilt files will be overwritten.
pub async fn update_limine() -> Result<(), Box<dyn Error>> {
    let limine_out_dir = Path::new(BUNDLED_DIR).join("limine");

    fs::create_dir_all(&limine_out_dir)?;

    let mut limine_clone_cmd = Command::new("git");

    limine_clone_cmd.arg("clone").arg(LIMINE_GITHUB_URL);
    limine_clone_cmd.arg("-b").arg(LIMINE_RELEASE_BRANCH);

    limine_clone_cmd.arg("bundled/limine");

    if !limine_clone_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", limine_clone_cmd))
        .success()
    {
        panic!("Failed to clone the latest prebuilt limine files")
    }

    Ok(())
}

/// Run [update_limine] if the limine prebuilt files do
/// not exist.
///
/// **Note**: To update the limine prebuilt files run `cargo boot update`.
pub async fn download_limine_prebuilt() -> Result<(), Box<dyn Error>> {
    let build_dir = Path::new(BUNDLED_DIR).join("limine");

    if !build_dir.exists() {
        update_limine().await?;

        return Ok(());
    }

    Ok(())
}

pub fn fetch() -> Result<(), Box<dyn Error>> {
    let bundled_dir = Path::new(BUNDLED_DIR).canonicalize()?;

    let mlibc_src_dir = bundled_dir.join("mlibc");

    if !mlibc_src_dir.exists() {
        xshell::cmd!("git clone --depth 1 --branch master https://github.com/Andy-Python-Programmer/mlibc bundled/mlibc").run()?;
    } else {
        let _p = xshell::pushd(&mlibc_src_dir)?;
        xshell::cmd!("git pull").run()?;
    }

    Ok(())
}

fn get_fat_filesystem_len(fat: Vec<&Path>) -> u64 {
    let mb = 1024 * 1024; // Size of a megabyte and round it to next megabyte.
    let mut size = 0x00;

    for file in fat {
        // Retrieve size of `path` file and round it up.
        let file_size = fs::metadata(file).unwrap().len();
        let file_size_rounded = ((file_size - 1) / mb + 1) * mb;

        size += file_size_rounded;
    }

    size
}

fn create_fat_filesystem(
    fat_path: &Path,
    efi_file: &Path,
    kernel_file: &Path,
    bootloader: AeroChainloader,
) -> Result<(), Box<dyn Error>> {
    let mut fat = vec![efi_file, kernel_file];

    if let AeroChainloader::Limine = bootloader {
        fat.push(&Path::new("bundled/limine/limine.sys"));
        fat.push(&Path::new("src/.cargo/limine.cfg"));
    }

    let fat_len = get_fat_filesystem_len(fat);

    // Create new filesystem image file at the given path and set its length.
    let fat_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&fat_path)?;

    fat_file.set_len(fat_len)?;

    // Create new FAT file system and open it.
    let format_options = fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32);
    fatfs::format_volume(&fat_file, format_options)?;

    let filesystem = fatfs::FileSystem::new(&fat_file, fatfs::FsOptions::new())?;

    // Copy EFI file to FAT filesystem.
    let root_dir = filesystem.root_dir();

    root_dir.create_dir("EFI")?;
    root_dir.create_dir("EFI/BOOT")?;
    root_dir.create_dir("EFI/KERNEL")?;

    macro_rules! create_fat_file {
        ($name:ident => $path:expr) => {
            let mut $name = root_dir.create_file($path)?;
            $name.truncate()?;
        };
    }

    macro_rules! copy_contents_fat {
        ($path:expr => $name:ident) => {
            io::copy(&mut fs::File::open($path)?, &mut $name)?
        };
    }

    create_fat_file!(bootx64 => "EFI/BOOT/BOOTX64.EFI");
    create_fat_file!(kernel => "aero_kernel.elf");

    copy_contents_fat!(&efi_file => bootx64);
    copy_contents_fat!(&kernel_file => kernel);

    if let AeroChainloader::Limine = bootloader {
        create_fat_file!(limine_sys => "limine.sys");
        create_fat_file!(limine_cfg => "limine.cfg");

        copy_contents_fat!("bundled/limine/limine.sys" => limine_sys);
        copy_contents_fat!("src/.cargo/limine.cfg" => limine_cfg);
    }

    Ok(())
}

fn create_gpt_disk(disk_path: &Path, fat_image: &Path) -> Result<(), Box<dyn Error>> {
    // Create new file.
    let mut disk = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&disk_path)?;

    // Set file size.
    let partition_size: u64 = fs::metadata(&fat_image)?.len();
    let disk_size = partition_size + 1024 * 64; // For GPT headers.
    disk.set_len(disk_size)?;

    /*
     * Create a protective MBR at LBA0 so that disk is not considered
     * unformatted on BIOS systems
     */
    let mbr = gpt::mbr::ProtectiveMBR::with_lb_size(
        u32::try_from((disk_size / 512) - 1).unwrap_or(0xFF_FF_FF_FF),
    );

    mbr.overwrite_lba0(&mut disk)?;

    // Create new GPT structure.
    let block_size = gpt::disk::LogicalBlockSize::Lb512;

    let mut gpt = gpt::GptConfig::new()
        .writable(true)
        .initialized(false)
        .logical_block_size(block_size)
        .create_from_device(Box::new(&mut disk), None)?;

    gpt.update_partitions(Default::default())?;

    // Add new EFI system partition and get its byte offset in the file.
    let partition_id = gpt.add_partition("boot", partition_size, gpt::partition_types::EFI, 0)?;

    let partition = gpt
        .partitions()
        .get(&partition_id)
        .expect("Boot partition not found");
    let start_offset = partition.bytes_start(block_size)?;

    // Close the GPT structure and flush out the changes.
    gpt.write()?;

    // Place the FAT filesystem in the newly created partition.
    disk.seek(io::SeekFrom::Start(start_offset))?;
    io::copy(&mut File::open(&fat_image)?, &mut disk)?;

    Ok(())
}

/// Packages all of the files by creating the build directory and copying
/// the `aero.elf` and the `BOOTX64.EFI` files to the build directory and creating
/// fat file from the build directory.
pub fn package_files(chainloader: AeroChainloader) -> Result<(), Box<dyn Error>> {
    let efi_file = Path::new("src/target/x86_64-unknown-uefi/debug/aero_boot.efi");

    let kernel_file = Path::new("src/target/x86_64-aero_os/debug/aero_kernel");
    let out_path = Path::new("build");

    let fat_path = out_path.join("aero.fat");
    let img_path = out_path.join("aero.img");

    fs::create_dir_all("build")?;

    create_fat_filesystem(&fat_path, &efi_file, &kernel_file, chainloader)?;
    create_gpt_disk(&img_path, &fat_path)?;

    if let AeroChainloader::Limine = chainloader {
        let mut limine_install_cmd = Command::new("bundled/limine/limine-install-linux-x86_64");
        limine_install_cmd.arg("build/aero.img");

        if !limine_install_cmd
            .status()
            .expect(&format!("Failed to run {:#?}", limine_install_cmd))
            .success()
        {
            panic!("Failed to install limine")
        }
    }

    Ok(())
}
