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

use aero_syscall::prelude::EPollEventFlags;
use alloc::sync::Arc;

use super::inode::{INodeInterface, PollTable};
use crate::utils::sync::{BlockQueue, Mutex};

pub struct EventFd {
    wq: BlockQueue,
    count: Mutex<usize>,
}

impl EventFd {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            wq: BlockQueue::new(),
            count: Mutex::new(0),
        })
    }
}

impl INodeInterface for EventFd {
    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> super::Result<usize> {
        self.wq.notify_complete();

        unimplemented!()
    }

    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> super::Result<usize> {
        self.wq.notify_complete();

        unimplemented!()
    }

    fn poll(&self, table: Option<&mut PollTable>) -> super::Result<EPollEventFlags> {
        let count = self.count.lock();
        let mut events = EPollEventFlags::default();

        table.map(|e| e.insert(&self.wq)); // listen for changes

        if *count > 0 {
            events.insert(EPollEventFlags::IN);
        }

        if *count == usize::MAX {
            events.insert(EPollEventFlags::ERR);
        }

        if *count < (usize::MAX - 1) {
            events.insert(EPollEventFlags::OUT); // possible to write a value of at least "1" without blocking.
        }

        Ok(events)
    }
}
