use alloc::{collections::VecDeque, sync::Arc};
use spin::mutex::spin::SpinMutex;

use crate::{
    arch::interrupts,
    userland::process::{Process, ProcessId},
    utils::PerCpu,
};

use super::{SchedulerInterface, PROCESS_CONTAINER};
use crate::userland::jump_userland;

#[thread_local]
static mut CURRENT_PROCESS: Option<Arc<SpinMutex<Process>>> = None;

#[thread_local]
static mut IDLE_THREAD: Option<Arc<SpinMutex<Process>>> = None;

fn set_current_process(process: &Arc<SpinMutex<Process>>) {
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
struct ProcessQueue(SpinMutex<VecDeque<ProcessId>>);

impl ProcessQueue {
    /// Creates a new process queue with no processes by default.
    #[inline]
    fn new() -> Self {
        Self(SpinMutex::new(VecDeque::new()))
    }

    /// Registers the provided `process` in the process queue.
    #[inline]
    fn register_process(&self, process_id: ProcessId) {
        self.0.lock().push_back(process_id);
    }

    fn front(&self) -> Option<ProcessId> {
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
        let idle_process = Process::new_idle();
        set_current_process(&idle_process);

        unsafe {
            interrupts::disable_interrupts();
            IDLE_THREAD = Some(idle_process);
            interrupts::enable_interrupts();
        }

        Arc::new(Self {
            queue: PerCpu::new(|| (SpinMutex::new(()), ProcessQueue::new())),
        })
    }
}

impl SchedulerInterface for RoundRobin {
    /// Registers the provided process into the process queue of this CPU.
    fn register_process(&self, process_id: ProcessId) {
        let (_, queue) = self.queue.get();

        queue.register_process(process_id);
    }

    fn reschedule(&self) -> bool {
        let (_, queue) = self.queue.get();

        let previous_process = unsafe {
            CURRENT_PROCESS
                .as_ref()
                .expect("`reschedule` was invoked with no active previous task")
                .clone()
        };

        let new_process = {
            match queue.front() {
                Some(new_process) => PROCESS_CONTAINER
                    .find_by_id(new_process)
                    .expect("Unknown process in queue"),

                None => unsafe {
                    IDLE_THREAD
                        .as_ref()
                        .expect("IDLE thread was not initialized")
                        .clone()
                },
            }
        };

        let new_process_lock = new_process.lock();
        let context = new_process_lock.get_context_ref();

        /*
         * Check if the pointer of the new process is the same as the new process. If thats
         * the case keep running the process and return out.
         */
        if Arc::ptr_eq(&new_process, &previous_process) {
            return false;
        }

        /*
         * Now that we have passed all of the checks, its time to run the actual process. We first
         * set the CURRENT_PROCESS static to the new process and then jump to ring 3. Leap of faith!
         */
        set_current_process(&new_process);

        unsafe {
            jump_userland(
                context.get_stack_top(),
                context.get_instruction_ptr(),
                context.rflags,
            );
        }

        true
    }
}

unsafe impl Send for RoundRobin {}
unsafe impl Sync for RoundRobin {}
