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

use aero_syscall::*;
use aipc_api::{system_server::SystemServer, window_server::WindowServer};

async fn discover_service(name: &str) -> Result<usize, AeroSyscallError> {
    let root_pid = sys_ipc_discover_root()?;
    let system = SystemServer::open(root_pid).await;

    system
        .discover(name)
        .await
        .map_err(|_| AeroSyscallError::ENOMSG)
}

fn main() {
    let mut rt = aipc::async_runtime::AsyncRuntime::new();

    rt.spawn(async {
        let window_server = WindowServer::open(discover_service("WindowServer").await.unwrap()).await;

        window_server.create_window("Test window 1").await;
        window_server.create_window("Test window 2").await;
        window_server.create_window("Test window 3").await;

        sys_exit(0);
    });
    rt.run();
}
