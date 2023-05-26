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
use aero_syscall::sys_ipc_discover_root;

fn main() {
    let self_pid = unsafe { libc::getpid() as usize };
    let ipc_root = sys_ipc_discover_root().unwrap();
    let system_client = SystemService::open(ipc_root);

    system_client.announce(self_pid, "WindowServer").unwrap();

    aero_ipc::listen(WindowService::handler(WindowServer));

    loop {
        aero_ipc::service_request();
    }
}

struct WindowServer;

impl WindowService::Server for WindowServer {
    fn create_window(&self, name: &str) -> usize {
        println!("[window_server] creating window with name: {}", name);

        0
    }
}
