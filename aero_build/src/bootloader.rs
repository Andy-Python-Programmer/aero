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

use std::process::Command;

use crate::CARGO;

pub fn build_bootloader() {
    println!("INFO: Building bootloader");

    let mut bootloader_build_cmd = Command::new(CARGO);

    bootloader_build_cmd.current_dir("src");

    bootloader_build_cmd.arg("build");
    bootloader_build_cmd.arg("--package").arg("aero_boot");

    bootloader_build_cmd
        .arg("--target")
        .arg("x86_64-unknown-uefi");

    if !bootloader_build_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", bootloader_build_cmd))
        .success()
    {
        panic!("Failed to build the bootloader")
    }
}
