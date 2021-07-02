/*
 * Copyright (C) 2021 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

use std::fs;

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::BUNDLED_DIR;

const PREBUILT_OVMF_URL: &str =
    "https://github.com/rust-osdev/ovmf-prebuilt/releases/latest/download/";

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

pub fn fetch() -> Result<(), Box<dyn Error>> {
    xshell::mkdir_p(BUNDLED_DIR)?;

    let bundled_dir = Path::new(BUNDLED_DIR).canonicalize()?;

    let mlibc_src_dir = bundled_dir.join("mlibc");
    let gcc_src_dir = bundled_dir.join("gcc");

    if !mlibc_src_dir.exists() {
        xshell::cmd!("git clone --depth 1 --branch master https://github.com/Andy-Python-Programmer/mlibc bundled/mlibc").run()?;
    }

    if !gcc_src_dir.exists() {
        xshell::cmd!("git clone --depth 1 --branch aero https://github.com/Andy-Python-Programmer/gcc bundled/gcc").run()?;
    }

    xshell::cmd!("chmod +x ./tools/setup_userland.sh").run()?;
    xshell::cmd!("./tools/setup_userland.sh").run()?;

    Ok(())
}

pub fn package_files(bios: Option<String>) -> Result<(), Box<dyn Error>> {
    xshell::cmd!("chmod +x ./tools/build_image.sh").run()?;

    match bios.as_deref() {
        Some("legacy") | None => xshell::cmd!("./tools/build_image.sh -b").run()?,
        Some("efi") => xshell::cmd!("./tools/build_image.sh -e").run()?,
        Some(_) => panic!()
    }

    Ok(())
}
