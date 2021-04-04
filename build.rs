use std::{env, error::Error, process::Command};

// fn assemble_trampoline() -> Result<(), Box<dyn Error>> {
//     let objects = nasm_rs::Build::new()
//         .file("src/acpi/trampoline.asm")
//         .flag("-f elf64")
//         .compile_objects()
//         .expect("NASM build failed. Make sure you have nasm installed. https://nasm.us");

//     let mut cc = cc::Build::new();

//     for o in objects {
//         cc.object(o);
//     }

//     cc.compile("trampoline");

//     Ok(())
// }

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

    if stdout != "" {
        panic!(
            "NASM build failed. Make sure you have nasm installed. https://nasm.us: {}",
            stdout
        );
    } else if stderr != "" {
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

    Ok(())
}
