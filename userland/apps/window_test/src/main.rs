// Copyright (C) 2021-2023 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use aero_ipc::{SystemService, WindowService};
use aero_syscall::{sys_ipc_discover_root, SyscallError};

fn discover_service(name: &str) -> Result<usize, SyscallError> {
    let root_pid = sys_ipc_discover_root()?;
    let system = SystemService::open(root_pid);

    system.discover(name).map_err(|_| SyscallError::ENOMSG)
}

fn main() -> Result<(), SyscallError> {
    let window_server = WindowService::open(discover_service("WindowServer")?);

    window_server.create_window("Test window 1");
    window_server.create_window("Test window 2");
    window_server.create_window("Test window 3");

    Ok(())
}
