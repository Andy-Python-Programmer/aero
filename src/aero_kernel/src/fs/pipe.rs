use alloc::{sync::Arc, vec::Vec};

use crate::utils::sync::{BlockQueue, Mutex};

use super::inode::INodeInterface;

struct Buffer {
    data: Vec<u8>,
}

impl Buffer {
    fn new() -> Self {
        Self { data: Vec::new() }
    }

    fn has_data(&self) -> bool {
        !self.data.is_empty()
    }

    fn read_data(&mut self, buffer: &mut [u8]) -> usize {
        // nothing to read mate
        if self.data.is_empty() {
            return 0;
        }

        let count = core::cmp::min(buffer.len(), self.data.len());

        for (i, b) in self.data.drain(..count).enumerate() {
            buffer[i] = b;
        }

        count
    }

    fn write_data(&mut self, data: &[u8]) -> usize {
        self.data.extend_from_slice(data);
        data.len() - 1
    }
}

struct BufferQueue {
    buffer: Mutex<Buffer>,
}

impl BufferQueue {
    fn new() -> Self {
        Self {
            buffer: Mutex::new(Buffer::new()),
        }
    }
}

pub struct Pipe {
    queue: BufferQueue,

    readers: BlockQueue,
    writers: BlockQueue,
}

impl Pipe {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            queue: BufferQueue::new(),

            readers: BlockQueue::new(),
            writers: BlockQueue::new(),
        })
    }
}

impl INodeInterface for Pipe {
    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> super::Result<usize> {
        let mut buffer = self
            .readers
            .block_on(&self.queue.buffer, |lock| lock.has_data());

        let read = buffer.read_data(buf);

        if read > 0 {
            // TODO: Notify only the first process
            self.writers.notify_complete();
        }

        Ok(read)
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> super::Result<usize> {
        let res = offset + self.queue.buffer.lock().write_data(buf);
        self.readers.notify_complete();

        Ok(res)
    }
}
