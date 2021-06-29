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

use std::env;
use std::error::Error;

fn assemble_trampoline(out_dir: &str) -> xshell::Result<()> {
    let trampoline_bin = format!("{}/smp_trampoline", out_dir);

    xshell::cmd!("nasm -f bin -o {trampoline_bin} src/acpi/trampoline.real.asm").run()
}

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = env::var("OUT_DIR")?;

    assemble_trampoline(&out_dir)?;

    println!("cargo:rerun-if-changed=.cargo/kernel.ld");

    Ok(())
}
