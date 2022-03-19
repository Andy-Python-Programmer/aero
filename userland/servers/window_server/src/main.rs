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
use aipc::async_runtime::Listener;

fn main() {
    let mut rt = aipc::async_runtime::AsyncRuntime::new();
    
    rt.spawn(async {
        let self_pid = sys_getpid().unwrap();
        let ipc_root = sys_ipc_discover_root().unwrap();
        let system_client = aipc_api::system_server::SystemServer::open(ipc_root).await;

        system_client
            .announce(self_pid, "WindowServer")
            .await
            .unwrap();
        
        ConcreteWindowServer::listen();
    });
    rt.run();
}

struct WindowServerData;

#[aipc::object(ConcreteWindowServer as aipc_api::window_server::WindowServer)]
impl WindowServerData {
    fn open() -> WindowServerData {
        WindowServerData
    }
    fn create_window(&self, name: &str) -> usize {
        println!("[window_server] creating window with name: {}", name);

        0
    }
}
