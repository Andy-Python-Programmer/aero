#![feature(decl_macro)]

use fs_extra::dir;
use fs_extra::dir::CopyOptions;

use structopt::StructOpt;

use std::fs;
use std::{env, time::Instant};

use std::error::Error;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// The cargo executable. This constant uses the `CARGO` environment variable to
/// also support non-standard cargo versions.
const CARGO: &str = env!("CARGO");

const BUNDLED_DIR: &str = "bundled";

mod bootloader;
mod bundled;

/// Build the kernel by using `cargo build` with the cargo config defined
/// in the `src\.cargo\config.toml` file.
fn build_kernel(target: Option<String>, bootloader: AeroBootloader) {
    println!("INFO: Building kernel");

    let mut kernel_build_cmd = Command::new(CARGO);

    kernel_build_cmd.current_dir("src");

    kernel_build_cmd.arg("build");
    kernel_build_cmd.arg("--package").arg("aero_kernel");

    if let AeroBootloader::Limine = bootloader {
        kernel_build_cmd.args(&["--features", "stivale2"]);
    }

    // Use the specified target. By default it will build for x86_64-aero_os
    if let Some(target) = target {
        kernel_build_cmd
            .arg("--target")
            .arg(format!("./.cargo/{}.json", target));
    } else {
        match bootloader {
            AeroBootloader::AeroBoot => {
                kernel_build_cmd
                    .arg("--target")
                    .arg("./.cargo/x86_64-aero_os.json");
            }

            AeroBootloader::Limine => {
                kernel_build_cmd
                    .arg("--target")
                    .arg("./aero_kernel/src/boot/stivale2/x86_64-aero_os.json");
            }
        }
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
    let mut qemu_run_cmd = Command::new("qemu-system-x86_64.exe");

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
    qemu_run_cmd.arg("-serial").arg("stdio");

    qemu_run_cmd
        .arg("-drive")
        .arg("format=raw,file=build/aero.img");

    qemu_run_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", qemu_run_cmd))
}

/// Build Aero's main webiste including its docs.
fn build_web() -> Result<(), Box<dyn Error>> {
    let mut docs_build_cmd = Command::new(CARGO);

    docs_build_cmd.current_dir("src");
    docs_build_cmd.arg("doc");

    // Generate the docs.
    if !docs_build_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", docs_build_cmd))
        .success()
    {
        panic!("Failed to build docs")
    }

    let cargo_output_dir = Path::new("src")
        .join("target")
        .join("x86_64-aero_os")
        .join("doc");

    let build_dir = Path::new("web").join("build");

    // Create the docs build directory.
    fs::create_dir_all(&build_dir)?;

    let mut cp_options = CopyOptions::new();
    cp_options.overwrite = true;

    // First move each file from the web/* directory to web/build/*
    for entry in fs::read_dir("web")? {
        let item = entry?;

        if item.file_type()?.is_file() {
            fs::copy(item.path(), build_dir.join(item.file_name()))?;
        }
    }

    // Now move all of the generated doc files by cargo to web/build/.
    dir::copy(cargo_output_dir, &build_dir, &cp_options)?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum AeroBootloader {
    AeroBoot,
    Limine,
}

impl From<Option<String>> for AeroBootloader {
    fn from(boot: Option<String>) -> Self {
        if let Some(boot) = boot {
            match boot.as_ref() {
                "aero" => Self::AeroBoot,
                "limine" => Self::Limine,
                _ => panic!("Invalid or unsupported bootloader {}", boot),
            }
        } else {
            Self::AeroBoot
        }
    }
}

#[derive(Debug, StructOpt)]
enum AeroBuildCommand {
    /// Build and run Aero in qemu.
    Run {
        #[structopt(long)]
        target: Option<String>,

        #[structopt(long)]
        chainloader: Option<String>,

        #[structopt(long)]
        bootloader: Option<String>,

        /// Extra command line arguments passed to qemu.
        #[structopt(last = true)]
        qemu_args: Vec<String>,
    },

    Build {
        #[structopt(long)]
        bootloader: Option<String>,

        #[structopt(long)]
        target: Option<String>,
    },

    /// Update all of the OVMF files required for UEFI and bootloader prebuilts.
    Update {
        #[structopt(long)]
        bootloader: Option<String>,
    },

    Web,
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
                bootloader,
                chainloader,
            } => {
                /*
                 * Get the current time. This is will be used to caclulate the build time
                 * after the build is finished.
                 */
                let now = Instant::now();
                let bootloader = AeroBootloader::from(bootloader);

                bundled::download_ovmf_prebuilt().await.unwrap();

                match bootloader {
                    AeroBootloader::AeroBoot => bootloader::build_bootloader(),
                    AeroBootloader::Limine => bundled::download_limine_prebuilt().await.unwrap(),
                }

                build_kernel(target, bootloader);
                bundled::package_files(bootloader).unwrap();

                println!("Build took {:?}", now.elapsed());

                if let Some(chainloader) = chainloader {
                    qemu_args.push("-drive".into());
                    qemu_args.push(format!("format=raw,file={}", chainloader));
                }

                if !run_qemu(qemu_args).success() {
                    panic!("Failed to run qemu");
                }
            }

            AeroBuildCommand::Build { bootloader, target } => {
                /*
                 * Get the current time. This is will be used to caclulate the build time
                 * after the build is finished.
                 */
                let now = Instant::now();
                let bootloader = AeroBootloader::from(bootloader);

                bundled::download_ovmf_prebuilt().await.unwrap();

                match bootloader {
                    AeroBootloader::AeroBoot => bootloader::build_bootloader(),
                    AeroBootloader::Limine => bundled::download_limine_prebuilt().await.unwrap(),
                }

                build_kernel(target, bootloader);
                bundled::package_files(bootloader).unwrap();

                println!("Build took {:?}", now.elapsed());
            }

            AeroBuildCommand::Update { bootloader } => {
                let bootloader = AeroBootloader::from(bootloader);

                bundled::update_ovmf()
                    .await
                    .expect("Failed tp update OVMF files");

                if let AeroBootloader::Limine = bootloader {
                    bundled::update_limine()
                        .await
                        .expect("Failed to update limine prebuilt files");
                }
            }

            AeroBuildCommand::Web => build_web().unwrap(),
        },

        None => {}
    }
}
