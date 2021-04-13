use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

fn assemble_assembly_source_files(dir: PathBuf) -> Result<(), Box<dyn Error>> {
    let read_dir = fs::read_dir(&dir)?;

    let mut nasm_build = nasm_rs::Build::new();
    let mut assembled = 0;

    for entry in read_dir {
        let entry = entry?;

        let file_name = &entry.file_name();
        let extension = Path::new(file_name).extension().and_then(OsStr::to_str);

        if entry.file_type()?.is_file() && extension == Some("asm") && file_name != "trampoline.asm"
        {
            nasm_build.file(entry.path());
            assembled += 1;
        } else if entry.file_type()?.is_dir() {
            assemble_assembly_source_files(entry.path())?;
        }
    }

    if assembled > 0 {
        nasm_build.flag("-f elf64");

        let objects = nasm_build
            .compile_objects()
            .expect("NASM build failed. Make sure you have nasm installed. https://nasm.us");

        let mut cc = cc::Build::new();

        for o in objects {
            cc.object(o);
        }

        cc.compile(dir.file_name().unwrap().to_str().unwrap());
    }

    Ok(())
}

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

    assemble_assembly_source_files(PathBuf::from_str("src")?)?;
    assemble_trampoline(&out_dir)?;

    Ok(())
}
