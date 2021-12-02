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

//! # Aero Build
//!
//! This module, is the implementation of the Aero build system. The goal of `aero_build` is to be an
//! easily understandable, easily extensible, and maintainable build system.

/*
 * Due to the requirement of building userland applications which require GCC and other unix-only tools
 * we do not support compilation on windows subsystems. We could easily support this by entering the WSL subsystem
 * when ever we want to run a unix-only tool but that would decrease the performence of compilation as we are writing
 * to windows drives. If you are using windows then its reccomended to use WSL 2 and clone aero in the WSL 2 file system
 * instead of the windows file system, due to the lack of performence. We do not want to waste precious time of the person
 * who is compiling aero on windows and just raise a compile error.
 */
#[cfg(target_family = "windows")]
compile_error!("aero does not support compilation on non-unix like systems");

const MAGIC_EXIT_STATUS_CODE_SUCCESS: i32 = 0x21;

use fs_extra::dir;
use fs_extra::dir::CopyOptions;

use structopt::StructOpt;

use std::env;
use std::fs;

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

/// The cargo executable. This constant uses the `CARGO` environment variable to
/// also support non-standard cargo versions.
const CARGO: &str = env!("CARGO");

const BUNDLED_DIR: &str = "bundled";

mod bundled;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BuildType {
    Debug,
    Release,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Bios {
    Legacy,
    Uefi,
}

impl From<Option<String>> for Bios {
    fn from(bios: Option<String>) -> Self {
        match bios.as_deref() {
            Some("legacy") | None => Self::Legacy,
            Some("uefi") => Self::Uefi,
            Some(v) => panic!("unsupported bios `{}`", v),
        }
    }
}

/// Extracts the built executable from output of `cargo build`.
fn extract_build_artifact(output: &str) -> anyhow::Result<Option<PathBuf>> {
    let json = json::parse(&output)?;

    // Get the executable path from the provided json output from
    // cargo.
    if let Some(executable) = json["executable"].as_str() {
        Ok(Some(PathBuf::from(executable)))
    } else {
        Ok(None)
    }
}

/// Builds the kernel with the provided target and returns the executable
/// path, or an error if the build failed.
fn build_kernel(
    target: Option<String>,
    build_type: BuildType,
    features: Vec<String>,
) -> anyhow::Result<PathBuf> {
    println!("INFO: Building kernel");

    let _p = xshell::pushd("src");

    // Use the provided target, or else use the default target.
    let target = target.unwrap_or(String::from("x86_64-aero_os"));
    let features = features.join(",");
    let mut command = xshell::cmd!(
        "{CARGO} build --package aero_kernel --target ./.cargo/{target}.json --message-format=json --features={features}"
    );

    if build_type == BuildType::Release {
        // FIXME: A simple workaround since xshell moves the value command when we
        // invoke the `arg` function.
        command = command.arg("--release");
    }

    let executable = command
        .read()?
        .lines()
        .map(extract_build_artifact)
        .take_while(Result::is_ok)
        .map(Result::unwrap)
        .filter(Option::is_some)
        .map(Option::unwrap)
        .next();

    if let Some(executable) = executable {
        Ok(executable)
    } else {
        // Error out if cargo did not provide us the build artifact.
        anyhow::bail!("no build atrifact found");
    }
}

/// Extracts the test built executable from output of `cargo test --no-run`.
fn extract_build_test_artifact(output: &str) -> anyhow::Result<Option<PathBuf>> {
    let json = json::parse(&output)?;

    // Get the executable path from the provided json output from
    // cargo.
    if let Some(executable) = json["executable"].as_str() {
        Ok(Some(PathBuf::from(executable)))
    } else {
        Ok(None)
    }
}

fn build_test_kernel(
    target: Option<String>,
    build_type: BuildType,
    features: Vec<String>,
) -> anyhow::Result<PathBuf> {
    println!("INFO: Building kernel");

    let _p = xshell::pushd("src");

    // Use the provided target, or else use the default target.
    let target = target.unwrap_or(String::from("x86_64-aero_os"));
    let features = features.join(",");
    let mut command = xshell::cmd!(
        "{CARGO} test --package aero_kernel --no-run --target ./.cargo/{target}.json --message-format=json --features={features}"
    );

    if build_type == BuildType::Release {
        // FIXME: A simple workaround since xshell moves the value command when we
        // invoke the `arg` function.
        command = command.arg("--release");
    }

    let executable = command
        .read()?
        .lines()
        .map(extract_build_test_artifact)
        .take_while(Result::is_ok)
        .map(Result::unwrap)
        .filter(Option::is_some)
        .map(Option::unwrap)
        .next();

    if let Some(executable) = executable {
        Ok(executable)
    } else {
        // Error out if cargo did not provide us the build artifact.
        anyhow::bail!("no build atrifact found");
    }
}

fn run_qemu(argv: Vec<String>, xserver: bool, bios: Bios) -> anyhow::Result<()> {
    // Calculate the qemu executable suffix.
    let qemu_suffix = if xserver && is_wsl() { "" } else { ".exe" };
    let qemu_exec = format!("qemu-system-x86_64{}", qemu_suffix);

    // Run the qemu executable. With the following default settings:
    //
    // - Set the machine type to q35.
    // - Set the CPU to the latest intel lake CPUs with 5 level paging support.
    // - Set the number of CPUs to 4.
    // - Set the amount of memory to 512MiB.
    // - Set serial port to qemu stdio.
    let mut command = Command::new(qemu_exec);
    let mut command = command
        .arg("-cpu")
        .arg("qemu64,+la57")
        .arg("-smp")
        .arg("1")
        .arg("-m")
        .arg("1G")
        .arg("-serial")
        .arg("stdio")
        .arg("-drive")
        .arg("format=raw,file=build/aero.img")
        .args(argv);

    if bios == Bios::Uefi {
        command = command
            .arg("-bios")
            .arg("bundled/ovmf/OVMF-pure-efi.fd")
            .arg("-machine")
            .arg("type=q35");
    }

    let status = command.status()?;
    let status_code = status.code().unwrap_or(1);

    assert!(status_code == 0 || status_code == MAGIC_EXIT_STATUS_CODE_SUCCESS);

    Ok(())
}

/// Builds and assembled the kernel and userland documentation into the web
/// build directory. This function will return an error if the documentation
/// build failed.
fn build_web(target: Option<String>) -> anyhow::Result<()> {
    let pushd = xshell::pushd("src");
    let target = target.unwrap_or(String::from("x86_64-aero_os"));

    xshell::cmd!("{CARGO} doc --target ./.cargo/{target}.json").run()?;

    core::mem::drop(pushd);

    let src_dir = PathBuf::from("src");
    let web_dir = PathBuf::from("web");

    let src_canonical = src_dir.canonicalize()?;
    let web_canonical = web_dir.canonicalize()?;

    let out = src_canonical.join("target").join(target).join("doc");
    let build_dir = web_canonical.join("build");

    // Create the docs build directory if it does not exist.
    fs::create_dir_all(&build_dir)?;

    let mut cp_options = CopyOptions::new();
    cp_options.overwrite = true;

    // First move each file from the web directory to the build directory.
    for entry in fs::read_dir("web")? {
        let item = entry?;

        if item.file_type()?.is_file() {
            fs::copy(item.path(), build_dir.join(item.file_name()))?;
        }
    }

    // Move all of the generated documentation files by cargo to the build directory.
    dir::copy(out, &build_dir, &cp_options)?;

    Ok(())
}

/// Builds all of the userland applications. This function will return an error
/// if the build failed.
fn build_userland() -> anyhow::Result<()> {
    println!("INFO: Building userland");
    let _p = xshell::pushd("userland");

    xshell::cmd!("{CARGO} build").run()?;

    Ok(())
}

#[derive(Debug, StructOpt)]
enum AeroBuildCommand {
    /// Builds and runs Aero in the provided `emulator`.
    Run {
        /// Sets the target triple to the provided `target`.
        #[structopt(long)]
        target: Option<String>,

        /// If set, the kernel will be compiled in release mode. If you are debugging
        /// this option should be set to false.
        #[structopt(short, long)]
        release: bool,

        /// Assembles the image with the provided BIOS. Possible options are
        /// `efi` and `legacy`. By default its set to `legacy`.
        #[structopt(long)]
        bios: Option<String>,

        /// If set, the the `emulator` executable will run in the linux subsystem
        /// and the user is required to launch an xserver in order to run the `emulator`.
        /// If using WSLG, the `xserver` argument must be set to true. On the other hand
        /// if you have the `emulator` installed in the windows subsystem, then set this
        /// argument to false (set by default).
        ///
        /// Note: On Linux, the `xserver` argument is ignored.
        #[structopt(short, long)]
        xserver: bool,

        #[structopt(short, long)]
        features: Vec<String>,

        /// Extra command line arguments to pass to the `emulator`.
        #[structopt(last = true)]
        qemu_args: Vec<String>,
    },

    Build {
        /// Sets the target triple to the provided `target`.
        #[structopt(long)]
        target: Option<String>,

        /// If set, the kernel will be compiled in release mode. If you are debugging
        /// this option should be set to false.
        #[structopt(short, long)]
        release: bool,

        #[structopt(short, long)]
        features: Vec<String>,

        /// Assembles the image with the provided BIOS. Possible options are
        /// `efi` and `legacy`. By default its set to `legacy`.
        #[structopt(long)]
        bios: Option<String>,
    },

    Test {
        /// Sets the target triple to the provided `target`.
        #[structopt(long)]
        target: Option<String>,

        /// If set, the the `emulator` executable will run in the linux subsystem
        /// and the user is required to launch an xserver in order to run the `emulator`.
        /// If using WSLG, the `xserver` argument must be set to true. On the other hand
        /// if you have the `emulator` installed in the windows subsystem, then set this
        /// argument to false (set by default).
        ///
        /// Note: On Linux, the `xserver` argument is ignored.
        #[structopt(short, long)]
        xserver: bool,

        #[structopt(short, long)]
        features: Vec<String>,

        /// Assembles the image with the provided BIOS. Possible options are
        /// `efi` and `legacy`. By default its set to `legacy`.
        #[structopt(long)]
        bios: Option<String>,

        /// Extra command line arguments to pass to the `emulator`.
        #[structopt(last = true)]
        qemu_args: Vec<String>,
    },

    /// Updates all of the build artifacts.
    Update,

    /// Cleans the build directory.
    Clean,

    /// Build the sysroot.
    Sysroot,

    /// Builds and assembles the documentation.
    Web {
        /// Sets the target triple to the provided `target`.
        #[structopt(long)]
        target: Option<String>,
    },
}

#[derive(Debug, StructOpt)]
struct AeroBuild {
    #[structopt(subcommand)]
    command: Option<AeroBuildCommand>,
}

/// Helper function to test if the host machine is WSL. For `aero_build` there are no
/// special requirements for WSL 2 but using WSL version 2 is recommended.
#[cfg(target_os = "linux")]
fn is_wsl() -> bool {
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
fn is_wsl() -> bool {
    false
}

fn main() -> anyhow::Result<()> {
    let aero_build = AeroBuild::from_args();

    match aero_build.command {
        Some(command) => match command {
            AeroBuildCommand::Run {
                qemu_args,
                target,
                xserver,
                release,
                features,
                bios,
            } => {
                let bios = Bios::from(bios);

                let build_type = if release {
                    BuildType::Release
                } else {
                    BuildType::Debug
                };

                xshell::mkdir_p(BUNDLED_DIR)?;

                // Get the current time. This is will be used to caclulate the build time
                // after the build is finished.
                let now = Instant::now();

                bundled::download_ovmf_prebuilt()?;

                build_userland()?;
                let path = build_kernel(target, build_type, features)?;

                bundled::package_files(bios, build_type, path)?;

                println!("Build took {:?}", now.elapsed());

                run_qemu(qemu_args, xserver, bios)?;
            }

            AeroBuildCommand::Test {
                target,
                features,
                bios,
                xserver,
                qemu_args,
            } => {
                let bios = Bios::from(bios);

                let build_type = BuildType::Debug;

                xshell::mkdir_p(BUNDLED_DIR)?;

                // Get the current time. This is will be used to caclulate the build time
                // after the build is finished.
                let now = Instant::now();

                bundled::download_ovmf_prebuilt()?;

                build_userland()?;

                let path = build_test_kernel(target, build_type, features)?;
                bundled::package_files(bios, build_type, path)?;

                println!("Build took {:?}", now.elapsed());
                run_qemu(qemu_args, xserver, bios)?;
            }

            AeroBuildCommand::Build {
                target,
                bios,
                features,
                release,
            } => {
                let bios = Bios::from(bios);

                let build_type = if release {
                    BuildType::Release
                } else {
                    BuildType::Debug
                };

                xshell::mkdir_p(BUNDLED_DIR)?;

                // Get the current time. This is will be used to caclulate the build time
                // after the build is finished.
                let now = Instant::now();

                bundled::download_ovmf_prebuilt()?;

                build_userland()?;
                let path = build_kernel(target, build_type, features)?;
                bundled::package_files(bios, build_type, path)?;

                println!("Build took {:?}", now.elapsed());
            }

            AeroBuildCommand::Update => {
                bundled::fetch()?;
                bundled::update_ovmf()?;
            }

            AeroBuildCommand::Clean => {
                xshell::rm_rf("./src/target")?;
                xshell::rm_rf("./userland/target")?;
            }

            AeroBuildCommand::Sysroot => bundled::fetch()?,
            AeroBuildCommand::Web { target } => build_web(target)?,
        },

        None => {}
    }

    Ok(())
}
