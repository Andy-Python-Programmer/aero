use alloc::collections::VecDeque;
use spin::{Mutex, MutexGuard, Once};

use super::process::Process;

static SCHEDULER: Once<Mutex<Scheduler>> = Once::new();

#[derive(Debug)]
pub struct Scheduler {
    pub processes: VecDeque<Process>,
}

impl Scheduler {
    /// Create a new scheduler with no active tasks by default.
    #[inline]
    fn new() -> Self {
        Self {
            processes: VecDeque::new(),
        }
    }

    #[inline]
    pub fn push(&mut self, process: Process) {
        self.processes.push_back(process);
    }

    #[inline]
    pub fn active_pid(&self) -> usize {
        self.processes
            .front()
            .expect("No processes running o_O")
            .pid
    }
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

/// Get a mutable reference to the active scheduler.
pub fn get_scheduler() -> MutexGuard<'static, Scheduler> {
    SCHEDULER
        .get()
        .expect("Attempted to get the scheduler before it was initialized")
        .lock()
}

pub fn reschedule() -> bool {
    true
}

/// Initialize the scheduler.
pub fn init() {
    let scheduler = Scheduler::new();

    SCHEDULER.call_once(move || Mutex::new(scheduler));
}
