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

use std::error::Error;
use std::fs::OpenOptions;
use std::process::Command;

use std::mem;

const DEFAULT_SHELL_PATH: &str = "/usr/bin/bash";
const TTY_PATH: &str = "/dev/tty";

fn main() -> Result<(), Box<dyn Error>> {
    // Open the stdin, stdout and stderr file descriptors.
    //
    // mem::forget(): don't drop the object which in turn will close the
    // file.
    mem::forget(OpenOptions::new().read(true).open(TTY_PATH)?); // fd=0 for stdin
    mem::forget(OpenOptions::new().write(true).open(TTY_PATH)?); // fd=1 for stdout
    mem::forget(OpenOptions::new().write(true).open(TTY_PATH)?); // fd=2 for stderr

    Command::new(DEFAULT_SHELL_PATH).spawn()?;

    Ok(())
}
