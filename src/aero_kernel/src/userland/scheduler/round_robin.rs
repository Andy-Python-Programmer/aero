use alloc::{collections::VecDeque, sync::Arc};
use spin::{mutex::spin::SpinMutex, Mutex};

use crate::{userland::process::Process, utils::PerCpu};

use super::SchedulerInterface;

/// Scheduler queue containing a vector of all of the process of the enqueued
/// processes.
#[repr(transparent)]
struct ProcessQueue(Mutex<VecDeque<Arc<Process>>>);

impl ProcessQueue {
    /// Creates a new process queue with no processes by default.
    #[inline]
    fn new() -> Self {
        Self(Mutex::new(VecDeque::new()))
    }

    /// Registers the provided `process` in the process queue.
    #[inline]
    fn register_process(&self, process: Arc<Process>) {
        self.0.lock().push_back(process);
    }

    fn front(&self) -> Option<Arc<Process>> {
        self.0.lock().pop_front()
    }
}

pub struct RoundRobin {
    queue: PerCpu<(SpinMutex<()>, ProcessQueue)>,
}

impl RoundRobin {
    /// Creates a new instance of the round robin scheduler and return a
    /// reference-counting pointer to itself. The task of this function
    /// is to initialize the per-cpu queues that the round robin scheduling
    /// algorithm requires.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            queue: PerCpu::new(|| (SpinMutex::new(()), ProcessQueue::new())),
        })
    }
}

impl SchedulerInterface for RoundRobin {
    fn register_process(&self, process: Arc<Process>) {
        let (_, queue) = self.queue.get();

        queue.register_process(process);
    }

    fn reschedule(&self) -> bool {
        let (_, queue) = self.queue.get();

        if let Some(process) = queue.front() {
            let context = process.get_context_ref();

            unsafe {
                super::super::jump_userland(
                    context.get_stack_top(),
                    context.get_instruction_ptr(),
                    context.rflags,
                );
            }

            true
        } else {
            false
        }
    }
}

unsafe impl Send for RoundRobin {}
unsafe impl Sync for RoundRobin {}
