use alloc::{collections::VecDeque, sync::Arc};
use spin::{mutex::spin::SpinMutex, Mutex};

use crate::{arch::interrupts, userland::process::Process, utils::PerCpu};

use super::SchedulerInterface;

#[thread_local]
static mut CURRENT_PROCESS: Option<Arc<Process>> = None;

fn set_current_process(process: &Arc<Process>) {
    // TODO(Andy-Python-Programmer): Instead of just disabling and enabling
    // interrupts, create an IRQ guard that stores if interrupt was enabled
    // and when the guard is dropped enable interupts if they were before.
    unsafe {
        interrupts::disable_interrupts();
        CURRENT_PROCESS = Some(process.clone());
        interrupts::enable_interrupts();
    }
}

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

/// Round Robin is the simplest algorithm for a preemptive scheduler. When the
/// system timer fires, the next process in the queue is switched to, and the
/// preempted process is put back into the queue.
///
/// ## Notes
/// * <https://en.wikipedia.org/wiki/Round-robin_scheduling>
pub struct RoundRobin {
    /// The per-cpu scheduler queues protected by a spin mutex.
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
    /// Registers the provided process into the process queue of this CPU.
    fn register_process(&self, process: Arc<Process>) {
        let (_, queue) = self.queue.get();

        set_current_process(&process);
        queue.register_process(process);
    }

    fn reschedule(&self) -> bool {
        let (_, queue) = self.queue.get();

        let previous_process = unsafe {
            CURRENT_PROCESS
                .as_ref()
                .expect("`reschedule` was invoked with no active previous task")
        };

        if let Some(new_process) = queue.front() {
            let context = new_process.get_context_ref();

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
