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

//this is all magic, yes dont ever let anyone see this shit

/// Helper function of walking the provided `dir`, only visiting files and calling
/// `cb` on each file.
fn visit_dirs(dir: &Path, cb: &mut dyn FnMut(&DirEntry)) -> std::io::Result<()> {
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
    let target = std::env::var("TARGET").expect("target triple is not set");

    if target.contains("aarch64") {
        return Ok(());
    }

    let mut inc_files = vec![];

    // Assemble all of the assembly real files first as they will be included in the
    // source files using `incbin`.
    visit_dirs(Path::new("src"), &mut |entry| {
        let path = entry.path();

        match path.extension() {
            Some(ext) if ext.eq(&OsString::from("inc")) => {
                let path = path
                    .to_str()
                    .expect("invalid UTF-8 for file path (skill issue)");
                inc_files.push(path.to_string())
            }

            _ => (),
        }
    })?;

    // more magic
    inc_files = inc_files
        .iter()
        .map(|e| {
            let e = e.split("/").collect::<Vec<_>>();
            e[..e.len() - 1].join("/").to_string()
        })
        .collect::<Vec<_>>();

    // Now that we have assembled all of the real files, we can go ahead and assemble the source
    // files.
    visit_dirs(Path::new("src"), &mut |entry: &DirEntry| {
        let path = entry.path();

        let object_os = path.file_name().expect("Failed to get file name");
        let object_file = object_os.to_str().expect("Invalid UTF-8 for file name");

        match path.extension() {
            Some(ext) if ext.eq(&OsString::from("asm")) => {
                let mut build = nasm_rs::Build::new();

                build
                    .file(&path)
                    .flag("-felf64")
                    .target("x86_64-unknown-none");

                println!("{:?}", inc_files);

                for include in &inc_files {
                    build.include(include);
                }

                build
                    .compile(object_file)
                    .expect("failed to compile assembly: skill issue");

                // Link it as a static library.
                println!("cargo:rustc-link-lib=static={}", object_file);
            }

            _ => (),
        }
    })?;

    println!("cargo:rerun-if-changed=.cargo/kernel.ld");

    Ok(())
}
