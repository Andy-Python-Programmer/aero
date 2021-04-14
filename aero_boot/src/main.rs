use structopt::StructOpt;

use std::env;
use std::process::{Command, ExitStatus};

/// The cargo executable. This constant uses the `CARGO` environment variable to
/// also support non-standard cargo versions.
const CARGO: &str = env!("CARGO");

/// The cargo home path. This constant uses the `CARGO_HOME` environment variable to
/// find the bootloader builder if its installed.
const CARGO_HOME: &str = env!("CARGO_HOME");

/// The qemu executable.
const QEMU: &str = "qemu-system-x86_64";

const BUNDLED_DIR: &str = "bundled";

mod bootloader;
mod uefi;
mod utils;

fn build_kernel() -> ExitStatus {
    println!("INFO: Building kernel");

    let mut kernel_build_command = Command::new(CARGO);

    kernel_build_command.current_dir("src");

    kernel_build_command.arg("build");
    kernel_build_command.arg("--package").arg("aero_kernel");

    kernel_build_command
        .status()
        .expect(&format!("Failed to run {:#?}", kernel_build_command))
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

#[derive(StructOpt, Debug)]
struct AeroBuild {
    #[structopt(short, long)]
    run: bool,

    /// Passing this flag will update all of the OVMF files required
    /// for UEFI.
    #[structopt(short, long)]
    update: bool,

    /// Extra command line arguments passed to qemu.
    #[structopt(last = true)]
    qemu_args: Vec<String>,
}

#[tokio::main]
async fn main() {
    let aero_build = AeroBuild::from_args();

    if aero_build.update {
        uefi::update_ovmf()
            .await
            .expect("Failed to update OVMF files");

        return;
    }

    if !build_kernel().success() {
        panic!("Failed to build the kernel");
    } else if !bootloader::build_bootloader().success() {
        panic!("Failed to build the bootloader");
    }

    uefi::download_ovmf_prebuilt().await.unwrap();

    if aero_build.run {
        if !run_qemu(aero_build.qemu_args).success() {
            panic!("Failed to run qemu");
        }
    }
}
