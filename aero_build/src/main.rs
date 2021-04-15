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

fn build_kernel() {
    println!("INFO: Building kernel");

    let mut kernel_build_command = Command::new(CARGO);

    kernel_build_command.current_dir("src");

    kernel_build_command.arg("build");
    kernel_build_command.arg("--package").arg("aero_kernel");

    if !kernel_build_command
        .status()
        .expect(&format!("Failed to run {:#?}", kernel_build_command))
        .success()
    {
        panic!("Failed to build the kernel")
    }
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

#[derive(Debug, StructOpt)]
enum AeroBuildCommand {
    /// Build and run Aero in qemu.
    Run {
        /// Extra command line arguments passed to qemu.
        #[structopt(last = true)]
        qemu_args: Vec<String>,
    },
    Build,
    /// Update all of the OVMF files required for UEFI.
    Update,
}

#[derive(Debug, StructOpt)]
struct AeroBuild {
    #[structopt(subcommand)]
    command: Option<AeroBuildCommand>,
}

#[tokio::main]
async fn main() {
    let aero_build = AeroBuild::from_args();

    match aero_build.command {
        Some(command) => match command {
            AeroBuildCommand::Run { qemu_args } => {
                uefi::download_ovmf_prebuilt().await.unwrap();

                build_kernel();
                bootloader::build_bootloader();

                if !run_qemu(qemu_args).success() {
                    panic!("Failed to run qemu");
                }
            }

            AeroBuildCommand::Build => {
                uefi::download_ovmf_prebuilt().await.unwrap();

                build_kernel();
                bootloader::build_bootloader();
            }

            AeroBuildCommand::Update => uefi::update_ovmf()
                .await
                .expect("Failed tp update OVMF files"),
        },
        None => {}
    }
}
