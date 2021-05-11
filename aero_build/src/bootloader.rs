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
