use alloc::collections::VecDeque;
use spin::{Mutex, MutexGuard, Once};

use super::process::Process;

static SCHEDULER: Once<Mutex<Scheduler>> = Once::new();

pub struct Scheduler {
    processes: VecDeque<Process>,
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
        let context = process.get_context_ref();

        let instruction_ptr = context.get_instruction_ptr();
        let stack_top = context.get_stack_top();
        let rflags = context.rflags;

        self.processes.push_back(process);

        unsafe {
            super::jump_userland(stack_top, instruction_ptr, rflags as usize);
        }
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
