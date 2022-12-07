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

use alloc::sync::Arc;

use super::inode::{INodeInterface, PollFlags, PollTable};
use crate::utils::sync::{BlockQueue, Mutex};

pub struct EventFd {
    wq: BlockQueue,
    count: Mutex<u64>,
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
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> super::Result<usize> {
        let size = core::mem::size_of::<u64>();
        assert!(buffer.len() == size);

        // SAFETY: We have above verified that it is safe to dereference
        //         the value.
        let value = unsafe { &mut *(buffer.as_mut_ptr() as *mut u64) };
        let mut count = self.wq.block_on(&self.count, |e| **e != 0)?;

        *value = *count;
        *count = 0; // reset the counter

        self.wq.notify_complete();
        Ok(size)
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> super::Result<usize> {
        let size = core::mem::size_of::<u64>();
        assert!(buffer.len() == size);

        // SAFETY: We have above verified that it is safe to dereference
        //         the value.
        let value = unsafe { *(buffer.as_ptr() as *const u64) };

        *self.count.lock_irq() += value;
        self.wq.notify_complete();
        Ok(size)
    }

    fn poll(&self, table: Option<&mut PollTable>) -> super::Result<PollFlags> {
        let count = self.count.lock();
        let mut events = PollFlags::empty();

        table.map(|e| e.insert(&self.wq)); // listen for changes

        if *count > 0 {
            events.insert(PollFlags::IN);
        }

        if *count == u64::MAX {
            events.insert(PollFlags::ERR);
        }

        if *count < (u64::MAX - 1) {
            events.insert(PollFlags::OUT); // possible to write a value of at least "1" without blocking.
        }

        Ok(events)
    }
}
