cfg_if::cfg_if! {
    if #[cfg(not(feature = "bin"))] {
        std::compile_error! {
            "The crate `aero_build` can only be compiled as a binary with the `bin` feature enabled."
        }
    }
}

use structopt::StructOpt;

use std::{
    env,
    fs::{self, File},
    io::Write,
};

use std::{
    error::Error,
    process::{Command, ExitStatus},
};

/// The cargo executable. This constant uses the `CARGO` environment variable to
/// also support non-standard cargo versions.
const CARGO: &str = env!("CARGO");

/// The qemu executable.
const QEMU: &str = "qemu-system-x86_64";

const BUNDLED_DIR: &str = "bundled";

mod bootloader;
mod uefi;

/// Build the kernel by using `cargo build` with the cargo config defined
/// in the `src\.cargo\config.toml` file.
fn build_kernel(target: Option<String>) {
    println!("INFO: Building kernel");

    let mut kernel_build_cmd = Command::new(CARGO);

    kernel_build_cmd.current_dir("src");

    kernel_build_cmd.arg("build");
    kernel_build_cmd.arg("--package").arg("aero_kernel");

    // Use the specified target. By default it will build for x86_64-aero_os
    if let Some(target) = target {
        kernel_build_cmd
            .arg("--target")
            .arg(format!("./.cargo/{}.json", target));
    }

    if !kernel_build_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", kernel_build_cmd))
        .success()
    {
        panic!("Failed to build the kernel")
    }
}

/// Runs Aero in qemu with UEFI as its default mode. By default it will
/// mount the build directory as a FAT partition instead of creating a seperate
/// `.fat` file. Check out [AeroBuild] for configuration settings about this.
fn run_qemu(argv: Vec<String>) -> ExitStatus {
    let mut qemu_run_cmd = Command::new(QEMU);

    qemu_run_cmd.args(argv);

    // Set up OVMF.
    qemu_run_cmd
        .arg("-drive")
        .arg("if=pflash,format=raw,file=bundled/ovmf/OVMF_CODE-pure-efi.fd");
    qemu_run_cmd
        .arg("-drive")
        .arg("if=pflash,format=raw,file=bundled/ovmf/OVMF_VARS-pure-efi.fd");
    qemu_run_cmd
        .arg("-bios")
        .arg("bundled/ovmf/OVMF-pure-efi.fd");

    qemu_run_cmd.arg("-machine").arg("type=q35");
    qemu_run_cmd.arg("-cpu").arg("qemu64");
    qemu_run_cmd.arg("-smp").arg("2");
    qemu_run_cmd.arg("-m").arg("512M");

    qemu_run_cmd
        .arg("-drive")
        .arg("format=raw,file=fat:rw:build/"); // Mounts the build directory as a FAT partition

    qemu_run_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", qemu_run_cmd))
}

/// Packages all of the files by creating the build directory and copying
/// the `aero.elf` and the `aero_boot.efi` files to the build directory and
/// creating the `startup.nsh` file.
fn package_files() -> Result<(), Box<dyn Error>> {
    // Create the build directory.
    fs::create_dir_all("build/efi/boot")?;
    fs::create_dir_all("build/efi/kernel")?;

    fs::copy(
        "src/target/x86_64-aero_os/debug/aero_kernel",
        "build/efi/kernel/aero.elf",
    )?;

    fs::copy(
        "src/target/x86_64-unknown-uefi/debug/aero_boot.efi",
        "build/efi/boot/aero_boot.efi",
    )?;

    // Create the `startup.nsh` file.
    let mut startup_nsh = File::create("build/startup.nsh")?;
    startup_nsh.write_all(br"\efi\boot\aero_boot.EFI")?;

    Ok(())
}

#[derive(Debug, StructOpt)]
enum AeroBuildCommand {
    /// Build and run Aero in qemu.
    Run {
        #[structopt(long)]
        target: Option<String>,

        #[structopt(long)]
        chainloader: Option<String>,

        /// Extra command line arguments passed to qemu.
        #[structopt(last = true)]
        qemu_args: Vec<String>,
    },
    Build {
        target: Option<String>,
    },
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
            AeroBuildCommand::Run {
                mut qemu_args,
                target,
                chainloader,
            } => {
                uefi::download_ovmf_prebuilt().await.unwrap();

                build_kernel(target);
                bootloader::build_bootloader();
                package_files().unwrap();

                if let Some(chainloader) = chainloader {
                    qemu_args.push("-drive".into());
                    qemu_args.push(format!("format=raw,file={}", chainloader));
                }

                if !run_qemu(qemu_args).success() {
                    panic!("Failed to run qemu");
                }
            }

            AeroBuildCommand::Build { target } => {
                uefi::download_ovmf_prebuilt().await.unwrap();

                build_kernel(target);
                bootloader::build_bootloader();
                package_files().unwrap();
            }

            AeroBuildCommand::Update => uefi::update_ovmf()
                .await
                .expect("Failed tp update OVMF files"),
        },

        None => {}
    }
}
