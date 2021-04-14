use std::env;
use std::path::Path;
use std::process::{Command, ExitStatus};

use crate::utils::locate_dependency_manifest;
use crate::{CARGO, CARGO_HOME};

pub fn build_bootloader() -> ExitStatus {
    println!("INFO: Building bootloader");

    let kernel_path = Path::new("src").join("aero_kernel");
    let bootloader_manifest = locate_dependency_manifest(&kernel_path, "bootloader").unwrap();

    let kernel_binary = Path::new("src/target/x86_64-aero_os/debug/aero_kernel")
        .canonicalize()
        .unwrap();

    let kernel_manifest = kernel_path.join("Cargo.toml").canonicalize().unwrap();

    let target_dir = Path::new("target");
    let out_dir = kernel_binary.parent().unwrap();

    let bootloader_builder = Path::new(CARGO_HOME)
        .join("bin")
        .join(format!("builder{}", env::consts::EXE_SUFFIX));

    if !bootloader_builder.exists() {
        if !install_bootloader_builder().success() {
            panic!("Failed to install bootloader builder.")
        }
    }

    let mut build_bootloader_cmd = Command::new(bootloader_builder);

    build_bootloader_cmd
        .arg("--kernel-manifest")
        .arg(&kernel_manifest);
    build_bootloader_cmd
        .arg("--kernel-binary")
        .arg(&kernel_binary);

    build_bootloader_cmd.arg("--target-dir").arg(&target_dir);
    build_bootloader_cmd.arg("--out-dir").arg(&out_dir);

    let bootloader_dir = bootloader_manifest.parent().unwrap();
    build_bootloader_cmd.current_dir(bootloader_dir);

    build_bootloader_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", build_bootloader_cmd))
}

fn install_bootloader_builder() -> ExitStatus {
    println!("INFO: Installing bootloader builder");

    let mut install_boot_command = Command::new(CARGO);

    install_boot_command.arg("install").arg("bootloader");
    install_boot_command.arg("--features").arg("builder");

    install_boot_command
        .status()
        .expect(&format!("Failed to run {:#?}", install_boot_command))
}
