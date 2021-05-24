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

use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

fn assemble_trampoline(out_dir: &str) -> Result<(), Box<dyn Error>> {
    let result = Command::new("nasm")
        .args(&[
            "-f",
            "bin",
            "-o",
            &format!("{}/trampoline", out_dir),
            "src/acpi/trampoline.asm",
        ])
        .output()?;

    let stdout = core::str::from_utf8(&result.stdout)?;
    let stderr = core::str::from_utf8(&result.stderr)?;

    if !stdout.is_empty() {
        panic!(
            "NASM build failed. Make sure you have nasm installed. https://nasm.us: {}",
            stdout
        );
    } else if !stderr.is_empty() {
        panic!(
            "NASM build failed. Make sure you have nasm installed. https://nasm.us: {}",
            stderr
        );
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let target_env = env::var("TARGET").expect("The target enviornment variable was not set");
    let target_triple = aero_build::target_triple!(target_env);

    let out_dir = env::var("OUT_DIR")?;

    aero_build::assemble_assembly_source_files(
        PathBuf::from_str("src")?,
        &target_triple,
        &vec!["trampoline.asm"],
    )?;

    assemble_trampoline(&out_dir)?;

    if cfg!(feature = "stivale2") {
        cc::Build::new()
            .file("./src/boot/stivale2/boot.c")
            .include("./src/boot/stivale2")
            .compile("stivale2_boot");
    }

    println!("cargo:rerun-if-changed=.cargo/kernel.ld");
    println!("cargo:rerun-if-changed=aero_kernel/src/boot/stivale2/kernel.ld");

    Ok(())
}
