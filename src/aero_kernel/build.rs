/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
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

use std::fs;

use std::error::Error;
use std::ffi::OsString;
use std::fs::DirEntry;
use std::path::Path;
use std::process::Command;

/// Helper function of walking the provided `dir`, only visiting files and calling
/// `cb` on each file.
fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // Assemble all of the assembly real files first as they will be included in the
    // source files using `incbin`.
    visit_dirs(Path::new("src"), &|entry| {
        let path = entry.path();

        match path.extension() {
            Some(ext) if ext.eq(&OsString::from("real")) => {
                let object_os = path.file_name().expect("Failed to get file name");
                let object_file = object_os.to_str().expect("Invalid UTF-8 for file name");

                let success = Command::new("nasm")
                    .arg("-f")
                    .arg("bin")
                    .arg("-o")
                    .arg(format!("../target/{}.bin", object_file))
                    .arg(format!("{}", path.display()))
                    .status()
                    .expect("Failed to assemble real source file")
                    .success();

                assert!(success);
            }

            _ => (),
        }
    })?;

    // Now that we have assembled all of the real files, we can go ahead and assemble the source
    // files.
    visit_dirs(Path::new("src"), &|entry: &DirEntry| {
        let path = entry.path();

        match path.extension() {
            Some(ext) if ext.eq(&OsString::from("asm")) => {
                let object_os = path.file_name().expect("Failed to get file name");
                let object_file = object_os.to_str().expect("Invalid UTF-8 for file name");

                nasm_rs::Build::new()
                    .file(&path)
                    .flag("-felf64")
                    .target("x86_64-unknown-none")
                    .compile(object_file)
                    .expect("Failed to compile assembly source file");

                // Link it as a static library.
                println!("cargo:rustc-link-lib=static={}", object_file);
            }

            _ => (),
        }
    })?;

    println!("cargo:rerun-if-changed=.cargo/kernel.ld");

    Ok(())
}
