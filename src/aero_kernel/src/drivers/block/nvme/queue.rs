use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU16, Ordering};

use crate::mem::paging::PhysAddr;

use super::command::{Command, CompletionEntry};
use super::dma::Dma;
use super::*;

const fn calculate_doorbell_offset(queue_id: u16, multiplier: usize, dstrd: usize) -> usize {
    0x1000 + ((((queue_id as usize) * 2) + multiplier) * (4 << dstrd))
}

pub struct Completion;
pub struct Submisson;

pub trait QueueType {
    type Type;
    const DOORBELL_OFFSET: usize;
}

impl QueueType for Completion {
    const DOORBELL_OFFSET: usize = 1;
    type Type = CompletionEntry;
}

impl QueueType for Submisson {
    const DOORBELL_OFFSET: usize = 0;
    type Type = Command;
}

#[repr(C)]
struct DoorBell(VolatileCell<u32>);

unsafe impl Send for DoorBell {}
unsafe impl Sync for DoorBell {}

pub(super) struct Queue<'bell, T: QueueType> {
    doorbell: &'bell DoorBell,
    index: usize,
    queue: Dma<[MaybeUninit<T::Type>]>,
    phase: bool,
}

impl<'bell, T: QueueType> Queue<'bell, T> {
    pub fn new(registers: &Registers, size: usize, queue_id: u16) -> Result<Self, Error> {
        let dstrd = registers.capability.get_doorbell_stride() as usize;
        let doorbell_offset = calculate_doorbell_offset(queue_id, T::DOORBELL_OFFSET, dstrd);

        let base_addr = registers as *const _ as usize;
        // SAFETY: The address is valid and aligned so, its safe to read from it.
        let doorbell = unsafe { &*((base_addr + doorbell_offset) as *const DoorBell) };

        Ok(Self {
            doorbell,
            queue: Dma::new_uninit_slice(size),
            index: 0,
            phase: true,
        })
    }

    pub fn addr(&self) -> PhysAddr {
        self.queue.addr()
    }
}

impl Queue<'_, Completion> {
    pub fn next_cmd_result(&mut self) -> Option<CompletionEntry> {
        let cur_completion = unsafe { self.queue[self.index].assume_init() };
        if cur_completion.get_phase_tag() != self.phase {
            self.index += 1;
            self.doorbell.0.set(self.index as u32);
            Some(cur_completion.clone())
        } else {
            None
        }
    }
}

impl Queue<'_, Submisson> {
    pub fn submit_command(&mut self, command: Command) {
        self.queue[self.index] = MaybeUninit::new(command);

        self.index += 1;
        self.doorbell.0.set(self.index as u32); // ring ring!
    }
}

static QUEUE_PAIR_ID: AtomicU16 = AtomicU16::new(0);

pub(super) struct QueuePair<'a> {
    id: u16,
    size: usize,

    submission: Queue<'a, Submisson>,
    completion: Queue<'a, Completion>,
}

impl<'a> QueuePair<'a> {
    pub fn new(registers: &Registers, size: usize) -> Result<Self, Error> {
        let queue_id = QUEUE_PAIR_ID.fetch_add(2, Ordering::SeqCst);

        Ok(Self {
            size,
            id: queue_id,

            submission: Queue::new(registers, size, queue_id)?,
            completion: Queue::new(registers, size, queue_id)?,
        })
    }

    pub fn submit_command<T: Into<Command>>(&mut self, command: T) {
        self.submission.submit_command(command.into());
        self.completion.next_cmd_result().unwrap();
    }

    /// Returns the physical address of the submission queue.
    pub fn submission_addr(&self) -> PhysAddr {
        self.submission.addr()
    }

    /// Returns the physical address of the completion queue.
    pub fn completion_addr(&self) -> PhysAddr {
        self.completion.addr()
    }

    /// Returns the unique ID of this queue pair.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// Returns the number of entries in the queue.
    pub fn len(&self) -> usize {
        self.size
    }
}
