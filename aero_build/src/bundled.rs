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
use std::path::{Path, PathBuf};

use crate::{Bios, BuildType, BUNDLED_DIR};

const PREBUILT_OVMF_URL: &str =
    "https://github.com/rust-osdev/ovmf-prebuilt/releases/latest/download/";

const OVMF_FILES: [&str; 1] = ["OVMF-pure-efi.fd"];

/// Download the latest release of the OVMF prebuilt files from `https://github.com/rust-osdev/ovmf-prebuilt`
/// and save them in the bundled/ovmf directory.
pub fn update_ovmf() -> anyhow::Result<()> {
    let bundled = PathBuf::from("./bundled");
    let bundled_canonical = bundled.canonicalize()?;

    let ovmf_out_dir = bundled_canonical.join("ovmf");

    fs::create_dir_all(&ovmf_out_dir)?; // Create the directory if it doesn't exist

    for ovmf_file in OVMF_FILES.iter() {
        println!("INFO: Downloading {}", ovmf_file);

        xshell::cmd!("curl")
            .arg("--location")
            .arg(format!("{}{}", PREBUILT_OVMF_URL, ovmf_file))
            .arg("--output")
            .arg(format!("{}", ovmf_out_dir.join(ovmf_file).display()))
            .run()?;
    }

    Ok(())
}

pub fn download_ovmf_prebuilt() -> anyhow::Result<()> {
    let ovmf_out_dir = Path::new(BUNDLED_DIR).join("ovmf");

    for ovmf_file in OVMF_FILES.iter() {
        if !ovmf_out_dir.join(ovmf_file).exists() {
            update_ovmf()?;

            return Ok(());
        }
    }

    Ok(())
}

pub fn fetch() -> anyhow::Result<()> {
    xshell::mkdir_p(BUNDLED_DIR)?;

    let bundled_dir = Path::new(BUNDLED_DIR).canonicalize()?;

    let mlibc_src_dir = bundled_dir.join("mlibc");
    let gcc_src_dir = bundled_dir.join("gcc");
    let binutils_src_dir = bundled_dir.join("binutils-gdb");

    if !mlibc_src_dir.exists() {
        xshell::cmd!("git clone --depth 1 --branch master https://github.com/Andy-Python-Programmer/mlibc bundled/mlibc").run()?;
    }

    if !gcc_src_dir.exists() {
        xshell::cmd!("git clone --depth 1 --branch aero https://github.com/Andy-Python-Programmer/gcc bundled/gcc").run()?;
    }

    if !binutils_src_dir.exists() {
        xshell::cmd!("git clone --depth 1 --branch aero https://github.com/Andy-Python-Programmer/binutils-gdb bundled/binutils-gdb").run()?;
    }

    xshell::cmd!("chmod +x ./tools/setup_userland.sh").run()?;
    xshell::cmd!("./tools/setup_userland.sh all").run()?;

    Ok(())
}

pub fn package_files(bios: Bios, mode: BuildType) -> anyhow::Result<()> {
    xshell::cmd!("chmod +x ./tools/build_image.sh").run()?;

    match (bios, mode) {
        (Bios::Legacy, BuildType::Debug) => xshell::cmd!("./tools/build_image.sh -b").run()?,
        (Bios::Legacy, BuildType::Release) => xshell::cmd!("./tools/build_image.sh -b")
            .env("RELEASE", "1")
            .run()?,

        (Bios::Uefi, BuildType::Debug) => xshell::cmd!("./tools/build_image.sh -e").run()?,
        (Bios::Uefi, BuildType::Release) => xshell::cmd!("./tools/build_image.sh -e")
            .env("RELEASE", "1")
            .run()?,
    }

    Ok(())
}
