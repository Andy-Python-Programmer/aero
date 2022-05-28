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

use aero_syscall::{prelude::EPollEvent, AeroSyscallError};
use alloc::sync::Arc;
use hashbrown::HashMap;

use crate::utils::sync::Mutex;

use super::inode::INodeInterface;

pub struct EPoll {
    events: Mutex<HashMap<usize, EPollEvent>>,
}

impl EPoll {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            events: Mutex::new(HashMap::new()),
        })
    }

    /// Adds an event to the epoll instance.
    ///
    /// ## Errors
    /// * `EEXIST`: The event already exists at `fd`.
    pub fn add_event(&self, fd: usize, event: EPollEvent) -> Result<(), AeroSyscallError> {
        let mut events = self.events.lock_irq();

        if events.get(&fd).is_some() {
            return Err(AeroSyscallError::EEXIST);
        }

        events.insert(fd, event);
        Ok(())
    }
}

unsafe impl Send for EPoll {}
unsafe impl Sync for EPoll {}

impl INodeInterface for EPoll {}
