use bootloader_locator::locate_bootloader;

use std::env;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// The cargo executable. This constant uses the `CARGO` environment variable to
/// also support non-standard cargo versions.
const CARGO: &str = env!("CARGO");

/// The cargo home path. This constant uses the `CARGO_HOME` environment variable to
/// find the bootloader builder if its installed.
const CARGO_HOME: &str = env!("CARGO_HOME");

/// The qemu executable.
const QEMU: &str = "qemu-system-x86_64";

fn build_kernel() -> ExitStatus {
    println!("INFO: Building kernel");

    let mut kernel_build_command = Command::new(CARGO);

    kernel_build_command.arg("build");
    kernel_build_command.arg("--package").arg("aero_kernel");

    kernel_build_command
        .status()
        .expect(&format!("Failed to run {:#?}", kernel_build_command))
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

fn build_bootloader() -> ExitStatus {
    println!("INFO: Building bootloader");

    env::set_current_dir("aero_kernel").unwrap();

    let bootloader_manifest = locate_bootloader("bootloader").unwrap();

    env::set_current_dir("..").unwrap();

    let kernel_binary = Path::new("target/x86_64-aero_os/debug/aero_kernel")
        .canonicalize()
        .unwrap();

    let kernel_manifest = Path::new("aero_kernel")
        .join("Cargo.toml")
        .canonicalize()
        .unwrap();

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

fn run_qemu(argv: Vec<String>) -> ExitStatus {
    let mut qemu_run_cmd = Command::new(QEMU);

    qemu_run_cmd.args(argv);

    // Set up OVMF.
    // qemu_run_cmd
    //     .arg("-drive")
    //     .arg("if=pflash,format=raw,file=bundled/ovmf/OVMF_CODE.fd");
    // qemu_run_cmd
    //     .arg("-drive")
    //     .arg("if=pflash,format=raw,file=bundled/ovmf/OVMF_VARS.fd");

    // qemu_run_cmd.arg("-machine").arg("q35");
    qemu_run_cmd
        .arg("-drive")
        .arg("format=raw,file=src/target/x86_64-aero_os/debug/boot-bios-aero_kernel.img");

    qemu_run_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", qemu_run_cmd))
}

fn main() {
    env::set_current_dir("src").unwrap();

    let mut argv = env::args().collect::<Vec<_>>()[1..].to_vec();

    if !build_kernel().success() {
        panic!("Failed to build the kernel");
    } else if !build_bootloader().success() {
        panic!("Failed to build the bootloader");
    }

    env::set_current_dir("..").unwrap();

    if argv.contains(&String::from("--run")) {
        // TODO: A better solution.
        let run_index = argv.iter().position(|x| *x == "--run").unwrap();
        argv.remove(run_index);

        if !run_qemu(argv).success() {
            panic!("Failed to run qemu");
        }
    }
}
