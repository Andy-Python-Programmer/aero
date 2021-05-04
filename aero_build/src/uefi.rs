use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use crate::{BUILD_DIR, BUNDLED_DIR};

const PREBUILT_OVMF_URL: &str =
    "https://github.com/rust-osdev/ovmf-prebuilt/releases/latest/download/";

const PREBUILT_LIMINE_URL: &str =
    "https://github.com/limine-bootloader/limine/releases/latest/download/";

const LIMINE_FILES: [&str; 1] = ["BOOTX64.EFI"];

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
    let build_dir = Path::new(BUILD_DIR).join("efi").join("boot");

    fs::create_dir_all(&build_dir)?;

    for lemon in LIMINE_FILES.iter() {
        println!("INFO: Downloading {}", lemon);

        let response = reqwest::get(format!("{}{}", PREBUILT_LIMINE_URL, lemon)).await?;
        let bytes = response.bytes().await?;

        let mut output = File::create(build_dir.join(lemon))?;
        output.write_all(bytes.as_ref())?;
    }

    Ok(())
}

/// Run [update_limine] if the limine prebuilt files do
/// not exist.
///
/// **Note**: To update the limine prebuilt files run `cargo boot update`.
pub async fn download_limine_prebuilt() -> Result<(), Box<dyn Error>> {
    let build_dir = Path::new(BUILD_DIR).join("efi").join("boot");

    for lemon in LIMINE_FILES.iter() {
        if !build_dir.join(lemon).exists() {
            update_limine().await?;

            return Ok(());
        }
    }

    Ok(())
}
