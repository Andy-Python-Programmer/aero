use core::sync::atomic::{AtomicUsize, Ordering};

use aero_syscall::OpenFlags;
use alloc::sync::Arc;

use crate::utils::buffer::Buffer;
use crate::utils::sync::{BlockQueue, Mutex};

use super::file_table::FileHandle;
use super::inode::{INodeInterface, PollFlags, PollTable};

pub struct Pipe {
    queue: Mutex<Buffer>,

    readers: BlockQueue,
    writers: BlockQueue,

    /// The number of writers currently connected to the pipe.
    num_writers: AtomicUsize,
}

impl Pipe {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            queue: Mutex::new(Buffer::new()),

            readers: BlockQueue::new(),
            writers: BlockQueue::new(),

            num_writers: AtomicUsize::new(0),
        })
    }

    /// Returns whether the pipe has active writers.
    pub fn has_active_writers(&self) -> usize {
        self.num_writers.load(Ordering::SeqCst)
    }
}

impl INodeInterface for Pipe {
    fn open(&self, flags: OpenFlags, _handle: Arc<FileHandle>) -> super::Result<()> {
        // Write end of the pipe:
        if flags.contains(OpenFlags::O_WRONLY) {
            self.num_writers.fetch_add(1, Ordering::SeqCst);
        }

        Ok(())
    }

    fn close(&self, flags: OpenFlags) {
        // Write end of the pipe:
        if flags.contains(OpenFlags::O_WRONLY) {
            let active_writers = self.num_writers.fetch_sub(1, Ordering::SeqCst) - 1;

            // There are no active writers and no data to read (reached EOF).
            if active_writers == 0 {
                self.readers.notify_complete();
            }
        }
    }

    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> super::Result<usize> {
        let mut buffer = self.readers.block_on(&self.queue, |lock| {
            lock.has_data() || !self.has_active_writers() == 0
        })?;

        let read = buffer.read_data(buf);

        if read > 0 {
            // TODO: Notify only the first process
            self.writers.notify_complete();
        }

        Ok(read)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> super::Result<usize> {
        let res = self.queue.lock_irq().write_data(buf);
        self.readers.notify_complete();

        Ok(res)
    }

    fn poll(&self, table: Option<&mut PollTable>) -> super::Result<PollFlags> {
        table.map(|e| {
            e.insert(&self.readers);
            e.insert(&self.writers)
        });

        let mut flags = PollFlags::OUT;

        if self.queue.lock_irq().has_data() {
            flags |= PollFlags::IN;
        }

        Ok(flags)
    }
}
