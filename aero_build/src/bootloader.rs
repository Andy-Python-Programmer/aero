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
