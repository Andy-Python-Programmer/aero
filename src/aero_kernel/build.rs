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

    println!("cargo:rerun-if-changed=.cargo/kernel.ld");
    println!("cargo:rerun-if-changed=aero_kernel/src/boot/stivale2/kernel.ld");

    Ok(())
}
