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
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::process::Command;

const TTY_PATH: &str = "/dev/tty";

fn remove_cloexec(file: &File) {
    // By default rust automatically sets the close-on-exe flag to prevent
    // leaking file descriptors.
    //
    // OpenOptions::custom_flags() only allows insertion of flags and are
    // overwritten by the flags set by the standard library. So here, we
    // need to manually update the flags after opening the file.
    let fd = file.as_raw_fd();

    unsafe {
        assert!(libc::fcntl(fd, libc::F_SETFD, 0 /* flags */) == 0);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Open the stdin, stdout and stderr file descriptors.
    let stdin = OpenOptions::new().read(true).open(TTY_PATH)?; // fd=0
    let stdout = OpenOptions::new().write(true).open(TTY_PATH)?; // fd=1
    let stderr = OpenOptions::new().write(true).open(TTY_PATH)?; // fd=2

    remove_cloexec(&stdin);
    remove_cloexec(&stdout);
    remove_cloexec(&stderr);

    Command::new("dhcpd").spawn()?;

    // Close the std{in,out,err} file descriptors, since now we are going to
    // start an X session.
    drop(stdin);
    drop(stdout);
    drop(stderr);

    Command::new("startx")
        .env("RUST_BACKTRACE", "full")
        .spawn()?;

    Ok(())
}
