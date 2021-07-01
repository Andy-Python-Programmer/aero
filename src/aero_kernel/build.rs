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

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    macro_rules! compile_asm {
        ($path:expr => $object:expr) => {
            nasm_rs::Build::new()
                .file($path)
                .flag("-felf64")
                .target("x86_64-unknown-none")
                .compile("smp_trampoline")?;
        };
    }

    xshell::cmd!("nasm -f bin -o ../target/smp_trampoline.bin src/acpi/trampoline.real.asm")
        .run()?;

    compile_asm!("src/acpi/smp_trampoline.asm" => "smp_trampoline");
    compile_asm!("src/arch/x86_64/task.asm" => "task");

    println!("cargo:rustc-link-lib=static=smp_trampoline");
    println!("cargo:rerun-if-changed=.cargo/kernel.ld");

    Ok(())
}
