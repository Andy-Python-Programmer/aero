use aipc::async_runtime::Listener;
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
use aipc_api::system_server::Error;
use hashbrown::{hash_map::Entry, HashMap};
use spin::{Once, RwLock};

// Basically the same thing that's in the init's main.rs
fn fork_and_exec(path: &str, argv: &[&str], envv: &[&str]) -> Result<usize, AeroSyscallError> {
    let pid = sys_fork()?;

    if pid == 0 {
        sys_exec(path, argv, envv)?;
        sys_exit(0);
    } else {
        Ok(pid)
    }
}

fn main() {
    sys_ipc_become_root().unwrap();

    fork_and_exec("/bin/window_server", &[], &[]).unwrap();

    let mut rt = aipc::async_runtime::AsyncRuntime::new();
    ConcreteSystemServer::listen();
    rt.run();
}

static SERVICES: Once<RwLock<HashMap<String, usize>>> = Once::new();

struct SystemServerData;

#[aipc::object(ConcreteSystemServer as aipc_api::system_server::SystemServer)]
impl SystemServerData {
    fn open() -> SystemServerData {
        SERVICES.call_once(|| RwLock::new(HashMap::new()));
        SystemServerData
    }

    fn announce(&self, pid: usize, name: &str) -> Result<(), Error> {
        let name = name.to_string();

        match SERVICES.get().unwrap().write().entry(name) {
            Entry::Occupied(_) => Err(Error::AlreadyProvided),
            Entry::Vacant(entry) => {
                entry.insert(pid);
                Ok(())
            }
        }
    }

    fn discover(&self, name: &str) -> Result<usize, Error> {
        let name = name.to_string();

        SERVICES
            .get()
            .unwrap()
            .read()
            .get(&name)
            .map(|pid| *pid)
            .ok_or(Error::NotFound)
    }
}
