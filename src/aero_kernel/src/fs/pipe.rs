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

use core::sync::atomic::{AtomicUsize, Ordering};

use aero_syscall::OpenFlags;
use alloc::sync::Arc;
use spin::Once;

use crate::utils::buffer::Buffer;
use crate::utils::sync::{Mutex, WaitQueue, WaitQueueFlags};

use super::cache::DirCacheItem;
use super::file_table::FileHandle;
use super::inode::{INodeInterface, PollFlags, PollTable};
use super::FileSystemError;

pub struct Pipe {
    queue: Mutex<Buffer>,

    readers: WaitQueue,
    writers: WaitQueue,

    /// The number of writers currently connected to the pipe.
    num_writers: AtomicUsize,
}

impl Pipe {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(Buffer::new()),

            readers: WaitQueue::new(),
            writers: WaitQueue::new(),

            num_writers: AtomicUsize::new(0),
        })
    }

    /// Returns the number of active writers to the pipe.
    pub fn active_writers(&self) -> usize {
        self.num_writers.load(Ordering::SeqCst)
    }
}

impl INodeInterface for Pipe {
    fn open(&self, handle: Arc<FileHandle>) -> super::Result<Option<DirCacheItem>> {
        // Write end of the pipe:
        if handle.flags().contains(OpenFlags::O_WRONLY) {
            self.num_writers.fetch_add(1, Ordering::SeqCst);
        }

        Ok(None)
    }

    fn close(&self, flags: OpenFlags) {
        // Write end of the pipe:
        if flags.contains(OpenFlags::O_WRONLY) {
            let active_writers = self.num_writers.fetch_sub(1, Ordering::SeqCst) - 1;

            // There are no active writers and no data to read (reached EOF).
            if active_writers == 0 {
                self.readers.notify_all();
            }
        }
    }

    fn read_at(&self, flags: OpenFlags, _offset: usize, buf: &mut [u8]) -> super::Result<usize> {
        if flags.is_nonblock() && !self.queue.lock_irq().has_data() {
            return Err(FileSystemError::WouldBlock);
        }

        let mut buffer = self.readers.wait(flags.into(), &self.queue, |lock| {
            lock.has_data() || self.active_writers() == 0
        })?;

        let read = buffer.read_data(buf);

        if read > 0 {
            // TODO: Notify only the first process
            self.writers.notify_all();
        }

        Ok(read)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> super::Result<usize> {
        let res = self.queue.lock_irq().write_data(buf);
        self.readers.notify_all();

        Ok(res)
    }

    fn poll(&self, table: Option<&mut PollTable>) -> super::Result<PollFlags> {
        if let Some(table) = table {
            table.insert(&self.readers);
            table.insert(&self.writers);
        }

        let mut flags = PollFlags::OUT;

        if self.queue.lock_irq().has_data() {
            flags |= PollFlags::IN;
        }

        Ok(flags)
    }
}
