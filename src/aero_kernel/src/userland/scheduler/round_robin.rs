use core::mem;

use alloc::{collections::VecDeque, sync::Arc};
use spin::mutex::spin::SpinMutex;

use crate::{
    userland::process::{Process, ProcessId},
    utils::{downcast, PerCpu},
};

use super::{SchedulerInterface, PROCESS_CONTAINER};
use crate::userland::jump_userland;

#[thread_local]
static mut CURRENT_PROCESS: Option<Arc<SpinMutex<Process>>> = None;

/// The kernel idle process is a special kind of process that is run when
/// no processes in the scheduler's queue are avaliable to execute. The idle process
/// is to be created for each CPU.
#[thread_local]
static mut IDLE_PROCESS: Option<Arc<SpinMutex<Process>>> = None;

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
    queue: PerCpu<ProcessQueue>,
}

impl RoundRobin {
    /// Creates a new instance of the round robin scheduler and return a
    /// reference-counting pointer to itself. The task of this function
    /// is to initialize the per-cpu queues that the round robin scheduling
    /// algorithm requires.
    pub fn new() -> Arc<Self> {
        let idle_process = Process::new_idle();

        unsafe {
            CURRENT_PROCESS = Some(idle_process.clone());
            IDLE_PROCESS = Some(idle_process);
        }

        Arc::new(Self {
            queue: PerCpu::new(|| ProcessQueue::new()),
        })
    }
}

impl SchedulerInterface for RoundRobin {
    /// Registers the provided process into the process queue of this CPU.
    fn register_process(&self, process_id: ProcessId) {
        let queue = self.queue.get();

        queue.register_process(process_id);
    }
}

unsafe impl Send for RoundRobin {}
unsafe impl Sync for RoundRobin {}

/// Yields execution to another process. The task of this function is to get the
/// process which is on the front of the process queue and jump to it. If no process are
/// avaviable for execution then the [IDLE_PROCESS] process is executed.
///
/// ## Overview
/// Instead of adding `reschedule` as a method in the [SchedulerInterface] trait we are making
/// this a normal function as in the trait case, the scheduler will be locked for a longer time. The
/// scheduler only needs lock protection for reserving the process id allocated.
pub fn reschedule() -> bool {
    let scheduler = super::get_scheduler();
    let round_robin: Option<Arc<RoundRobin>> = downcast(&scheduler.inner);
    let scheduler_ref = round_robin.expect("Failed to downcast the scheduler");

    let queue = scheduler_ref.queue.get();

    mem::drop(scheduler); // Unlock the scheduler

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
                IDLE_PROCESS
                    .as_ref()
                    .expect("IDLE thread was not initialized")
                    .clone()
            },
        }
    };

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
    unsafe {
        CURRENT_PROCESS = Some(new_process.clone());
    }

    let new_process = new_process.lock();
    let context = new_process.get_context_ref();

    if let Some(_) = new_process.address_space.as_ref() {
        // TODO(Andy-Python-Programmer): Switch to the new user page table. Also map the user stack
        // in the address space when creating the process itself.
    }

    unsafe {
        jump_userland(
            context.get_stack_top(),
            context.get_instruction_ptr(),
            context.rflags,
        );
    }

    true
}
