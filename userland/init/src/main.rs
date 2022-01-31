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

use aero_ipc::event_loop::EventLoop;
use aero_syscall::*;

fn fork_and_exec(path: &str, argv: &[&str], envv: &[&str]) -> Result<(), AeroSyscallError> {
    let pid = sys_fork()?;

    if pid == 0 {
        sys_exec(path, argv, envv)?;
        sys_exit(1); // Unreachable
    } else {
        Ok(())
    }
}

fn main() -> Result<(), AeroSyscallError> {
    // Open the stdio file descriptors.
    sys_open("/dev/tty", OpenFlags::O_RDONLY)?;
    sys_open("/dev/tty", OpenFlags::O_WRONLY)?;
    sys_open("/dev/tty", OpenFlags::O_WRONLY)?;

    // Run some child processes
    fork_and_exec("/bin/aero_shell", &[], &[])?;

    // Create the event loop and handle the events
    let mut event_loop = EventLoop::new();

    loop {
        let message = event_loop.receive()?;

        println!("{:?}", message);
    }
}
