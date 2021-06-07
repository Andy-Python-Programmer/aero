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

//! # Aero Build
//!
//! This module, is the implementation of the Aero build system. The goal of `aero_build` is to be an
//! easily understandable, easily extensible, and maintainable build system.

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
fn build_kernel(target: Option<String>, bootloader: AeroChainloader) {
    println!("INFO: Building kernel");

    let mut kernel_build_cmd = Command::new(CARGO);

    kernel_build_cmd.current_dir("src");

    kernel_build_cmd.arg("build");
    kernel_build_cmd.arg("--package").arg("aero_kernel");

    if let AeroChainloader::Limine = bootloader {
        kernel_build_cmd.args(&["--features", "stivale2"]);
    }

    // Use the specified target. By default it will build for x86_64-aero_os
    if let Some(target) = target {
        kernel_build_cmd
            .arg("--target")
            .arg(format!("./.cargo/{}.json", target));
    } else {
        kernel_build_cmd
            .arg("--target")
            .arg("./.cargo/x86_64-aero_os.json");
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
    let mut qemu_run_cmd = Command::new(format!(
        "qemu-system-x86_64{}",
        if is_wsl() { ".exe" } else { "" }
    ));

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

fn build_userland() {
    println!("INFO: Building userland");

    let mut build_userland_cmd = Command::new(CARGO);

    build_userland_cmd.current_dir("userland");
    build_userland_cmd.arg("build");

    if !build_userland_cmd
        .status()
        .expect(&format!("Failed to run {:#?}", build_userland_cmd))
        .success()
    {
        panic!("Failed to build docs")
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AeroChainloader {
    Limine,
    None,
}

impl From<Option<String>> for AeroChainloader {
    fn from(boot: Option<String>) -> Self {
        if let Some(boot) = boot {
            match boot.as_ref() {
                "limine" => Self::Limine,
                _ => panic!("Invalid or unsupported bootloader {}", boot),
            }
        } else {
            Self::None
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

        /// Extra command line arguments passed to qemu.
        #[structopt(last = true)]
        qemu_args: Vec<String>,
    },

    Build {
        #[structopt(long)]
        chainloader: Option<String>,

        #[structopt(long)]
        target: Option<String>,
    },

    /// Update all of the OVMF files required for UEFI and bootloader prebuilts.
    Update {
        #[structopt(long)]
        chainloader: Option<String>,
    },

    Web,
}

#[derive(Debug, StructOpt)]
struct AeroBuild {
    #[structopt(subcommand)]
    command: Option<AeroBuildCommand>,
}

/// Helper function to test if the host machine is WSL. For `aero_build` there are no
/// special requirements for WSL 2 but using WSL version 2 is recommended.
#[cfg(target_os = "linux")]
pub fn is_wsl() -> bool {
    if let Ok(info) = std::fs::read("/proc/sys/kernel/osrelease") {
        if let Ok(info_str) = std::str::from_utf8(&info) {
            let info_str = info_str.to_ascii_lowercase();
            return info_str.contains("microsoft") || info_str.contains("wsl");
        }
    }

    false
}

/// Helper function to test if the host machine is WSL. For `aero_build` there are no
/// special requirements for WSL 2 but using WSL version 2 is recommended.
#[cfg(not(target_os = "linux"))]
pub fn is_wsl() -> bool {
    false
}

#[tokio::main]
async fn main() {
    let aero_build = AeroBuild::from_args();

    match aero_build.command {
        Some(command) => match command {
            AeroBuildCommand::Run {
                qemu_args,
                target,
                chainloader,
            } => {
                bundled::fetch().unwrap();

                /*
                 * Get the current time. This is will be used to caclulate the build time
                 * after the build is finished.
                 */
                let now = Instant::now();
                let chainloader = AeroChainloader::from(chainloader);

                bundled::download_ovmf_prebuilt().await.unwrap();
                bootloader::build_bootloader();

                match chainloader {
                    AeroChainloader::Limine => bundled::download_limine_prebuilt().await.unwrap(),
                    AeroChainloader::None => {}
                }

                build_userland();
                build_kernel(target, chainloader);
                bundled::package_files(chainloader).unwrap();

                println!("Build took {:?}", now.elapsed());

                if !run_qemu(qemu_args).success() {
                    panic!("Failed to run qemu");
                }
            }

            AeroBuildCommand::Build {
                chainloader,
                target,
            } => {
                bundled::fetch().unwrap();

                /*
                 * Get the current time. This is will be used to caclulate the build time
                 * after the build is finished.
                 */
                let now = Instant::now();
                let chainloader = AeroChainloader::from(chainloader);

                bundled::download_ovmf_prebuilt().await.unwrap();
                bootloader::build_bootloader();

                match chainloader {
                    AeroChainloader::Limine => bundled::download_limine_prebuilt().await.unwrap(),
                    AeroChainloader::None => {}
                }

                build_userland();
                build_kernel(target, chainloader);
                bundled::package_files(chainloader).unwrap();

                println!("Build took {:?}", now.elapsed());
            }

            AeroBuildCommand::Update { chainloader } => {
                bundled::fetch().unwrap();

                let chainloader = AeroChainloader::from(chainloader);

                bundled::update_ovmf()
                    .await
                    .expect("Failed tp update OVMF files");

                if let AeroChainloader::Limine = chainloader {
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
