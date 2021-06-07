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
use std::process::Command;

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
    let out_dir = env::var("OUT_DIR")?;
    assemble_trampoline(&out_dir)?;

    println!("cargo:rerun-if-changed=.cargo/kernel.ld");

    Ok(())
}
