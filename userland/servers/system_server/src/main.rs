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

use aero_ipc::{SystemService, SystemServiceError, SystemServiceResult};
use aero_syscall::*;
use hashbrown::{hash_map::Entry, HashMap};
use spin::RwLock;

// Basically the same thing that's in the init's main.rs
fn fork_and_exec(path: &str, argv: &[&str], envv: &[&str]) -> Result<usize, AeroSyscallError> {
    let pid = sys_fork()?;

    if pid == 0 {
        sys_execve(path, argv, envv)?;
        sys_exit(0);
    } else {
        Ok(pid)
    }
}

fn main() {
    sys_ipc_become_root().unwrap();

    aero_ipc::listen(SystemService::handler(SystemServer::new()));

    fork_and_exec("/bin/window_server", &[], &[]).unwrap();

    loop {
        aero_ipc::service_request();
    }
}

struct SystemServer {
    services: RwLock<HashMap<String, usize>>,
}

impl SystemServer {
    fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::with_capacity(24)),
        }
    }
}

impl SystemService::Server for SystemServer {
    fn announce(&self, pid: usize, name: &str) -> SystemServiceResult<()> {
        let name = name.to_string();

        match self.services.write().entry(name) {
            Entry::Occupied(_) => Err(SystemServiceError::AlreadyProvided),
            Entry::Vacant(entry) => {
                entry.insert(pid);
                Ok(())
            }
        }
    }

    fn discover(&self, name: &str) -> SystemServiceResult<usize> {
        let name = name.to_string();

        self.services
            .read()
            .get(&name)
            .map(|pid| *pid)
            .ok_or(SystemServiceError::NotFound)
    }
}
