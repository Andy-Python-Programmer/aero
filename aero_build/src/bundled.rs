use std::fs;
use std::io;

use std::error::Error;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use std::process::Command;

use std::convert::TryFrom;

use crate::{AeroBootloader, BUNDLED_DIR};

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

fn get_fat_filesystem_len(paths: &[&Path]) -> u64 {
    let mb = 1024 * 1024; // Size of a megabyte and round it to next megabyte.
    let mut size = 0x00;

    for path in paths {
        // Retrieve size of `path` file and round it up.
        let file_size = fs::metadata(path).unwrap().len();
        let file_size_rounded = ((file_size - 1) / mb + 1) * mb;

        size += file_size_rounded;
    }

    size
}

fn create_fat_filesystem(
    fat_path: &Path,
    efi_file: &Path,
    kernel_file: &Path,
    bootloader: AeroBootloader,
) {
    let fat_len = get_fat_filesystem_len(&[
        efi_file,
        kernel_file,
        &Path::new("bundled/limine/limine.sys"),
        &Path::new("src/.cargo/limine.cfg"),
    ]);

    // Create new filesystem image file at the given path and set its length.
    let fat_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&fat_path)
        .unwrap();

    fat_file.set_len(fat_len).unwrap();

    // Create new FAT file system and open it.
    let format_options = fatfs::FormatVolumeOptions::new();
    fatfs::format_volume(&fat_file, format_options).unwrap();

    let filesystem = fatfs::FileSystem::new(&fat_file, fatfs::FsOptions::new()).unwrap();

    // Copy EFI file to FAT filesystem.
    let root_dir = filesystem.root_dir();

    root_dir.create_dir("EFI").unwrap();
    root_dir.create_dir("EFI/BOOT").unwrap();
    root_dir.create_dir("EFI/KERNEL").unwrap();

    let mut bootx64 = root_dir.create_file("EFI/BOOT/BOOTX64.EFI").unwrap();
    bootx64.truncate().unwrap();

    let mut kernel = root_dir.create_file("EFI/KERNEL/aero_kernel.elf").unwrap();
    kernel.truncate().unwrap();

    if let AeroBootloader::Limine = bootloader {
        let mut limine_sys = root_dir.create_file("limine.sys").unwrap();
        limine_sys.truncate().unwrap();

        let mut limine_cfg = root_dir.create_file("limine.cfg").unwrap();
        limine_cfg.truncate().unwrap();

        io::copy(
            &mut fs::File::open("bundled/limine/limine.sys").unwrap(),
            &mut limine_sys,
        )
        .unwrap();

        io::copy(
            &mut fs::File::open("src/.cargo/limine.cfg").unwrap(),
            &mut limine_cfg,
        )
        .unwrap();
    }

    io::copy(&mut fs::File::open(&efi_file).unwrap(), &mut bootx64).unwrap();
    io::copy(&mut fs::File::open(&kernel_file).unwrap(), &mut kernel).unwrap();
}

fn create_gpt_disk(disk_path: &Path, fat_image: &Path) {
    // Create new file.
    let mut disk = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&disk_path)
        .unwrap();

    // Set file size.
    let partition_size: u64 = fs::metadata(&fat_image).unwrap().len();
    let disk_size = partition_size + 1024 * 64; // For GPT headers.
    disk.set_len(disk_size).unwrap();

    /*
     * Create a protective MBR at LBA0 so that disk is not considered
     * unformatted on BIOS systems
     */
    let mbr = gpt::mbr::ProtectiveMBR::with_lb_size(
        u32::try_from((disk_size / 512) - 1).unwrap_or(0xFF_FF_FF_FF),
    );

    mbr.overwrite_lba0(&mut disk).unwrap();

    // Create new GPT structure.
    let block_size = gpt::disk::LogicalBlockSize::Lb512;
    let mut gpt = gpt::GptConfig::new()
        .writable(true)
        .initialized(false)
        .logical_block_size(block_size)
        .create_from_device(Box::new(&mut disk), None)
        .unwrap();
    gpt.update_partitions(Default::default()).unwrap();

    // Add new EFI system partition and get its byte offset in the file.
    let partition_id = gpt
        .add_partition("boot", partition_size, gpt::partition_types::EFI, 0)
        .unwrap();

    let partition = gpt.partitions().get(&partition_id).unwrap();
    let start_offset = partition.bytes_start(block_size).unwrap();

    // Close the GPT structure and flush out the changes.
    gpt.write().unwrap();

    // Place the FAT filesystem in the newly created partition.
    disk.seek(io::SeekFrom::Start(start_offset)).unwrap();

    io::copy(&mut File::open(&fat_image).unwrap(), &mut disk).unwrap();
}

/// Packages all of the files by creating the build directory and copying
/// the `aero.elf` and the `BOOTX64.EFI` files to the build directory and creating
/// fat file from the build directory.
pub fn package_files(bootloader: AeroBootloader) -> Result<(), Box<dyn Error>> {
    let efi_file = match bootloader {
        AeroBootloader::AeroBoot => Path::new("src/target/x86_64-unknown-uefi/debug/aero_boot.efi"),
        AeroBootloader::Limine => Path::new("bundled/limine/BOOTX64.EFI"),

        AeroBootloader::Tomato => Path::new(""),
        AeroBootloader::Multiboot2 => Path::new(""),
    };

    let kernel_file = Path::new("src/target/x86_64-aero_os/debug/aero_kernel");
    let out_path = Path::new("build");

    let fat_path = out_path.join("aero.fat");
    let img_path = out_path.join("aero.img");

    fs::create_dir_all("build")?;

    create_fat_filesystem(&fat_path, &efi_file, &kernel_file, bootloader);
    create_gpt_disk(&img_path, &fat_path);

    if let AeroBootloader::Limine = bootloader {
        let mut limine_install_cmd = Command::new("wsl");

        limine_install_cmd.arg("bundled/limine/limine-install-linux-x86_64");
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
