// Copyright (C) 2021-2024 The Aero Project Developers.
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

use aero_syscall::OpenFlags;
use alloc::sync::Arc;
use spin::Once;

use super::file_table::FileHandle;
use super::inode::{INodeInterface, PollFlags, PollTable};
use crate::fs::FileSystemError;
use crate::utils::sync::{Mutex, WaitQueue};

pub struct EventFd {
    wq: WaitQueue,
    /// Every write(2) on an eventfd, the value written is added to `count` and a wakeup
    /// is performed on `wq`.
    count: Mutex<u64>,
    // FIXME: https://github.com/Andy-Python-Programmer/aero/issues/113
    handle: Once<Arc<FileHandle>>,
}

impl EventFd {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            wq: WaitQueue::new(),
            count: Mutex::new(0),
            handle: Once::new(),
        })
    }

    fn is_nonblock(&self) -> bool {
        let handle = self.handle.get().expect("file handle is not initialized");
        handle.flags().contains(OpenFlags::O_NONBLOCK)
    }
}

impl INodeInterface for EventFd {
    fn open(&self, handle: Arc<FileHandle>) -> super::Result<Option<super::cache::DirCacheItem>> {
        self.handle.call_once(|| handle);
        Ok(None)
    }

    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> super::Result<usize> {
        let size = core::mem::size_of::<u64>();
        assert!(buffer.len() >= size);

        // SAFETY: We have above verified that it is safe to dereference
        //         the value.
        let value = unsafe { &mut *(buffer.as_mut_ptr().cast::<u64>()) };
        let mut count = self.wq.block_on(&self.count, |e| **e != 0)?;

        *value = *count;
        *count = 0; // reset the counter

        self.wq.notify_all();
        Ok(size)
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> super::Result<usize> {
        let chunk_size = core::mem::size_of::<u64>();
        assert!(buffer.len() >= chunk_size);

        // TODO: use bytemuck to remove the unsafe.
        let target = unsafe { *(buffer.as_ptr().cast::<u64>()) };

        if target == u64::MAX {
            return Err(FileSystemError::NotSupported);
        }

        let mut count = self.count.lock();

        if u64::MAX - *count > target {
            *count += target;
        } else if !self.is_nonblock() {
            unimplemented!()
        } else {
            return Ok(0);
        };

        self.wq.notify_all();
        Ok(chunk_size)
    }

    fn poll(&self, table: Option<&mut PollTable>) -> super::Result<PollFlags> {
        let count = self.count.lock_irq();
        let mut events = PollFlags::empty();

        if let Some(e) = table {
            e.insert(&self.wq)
        }

        if *count > 0 {
            events.insert(PollFlags::IN);
        }

        if *count == u64::MAX {
            events.insert(PollFlags::ERR);
        }

        if *count < (u64::MAX - 1) {
            events.insert(PollFlags::OUT); // possible to write a value of at least "1" without
                                           // blocking.
        }

        Ok(events)
    }
}
