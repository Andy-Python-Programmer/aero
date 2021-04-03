use alloc::collections::VecDeque;
use lazy_static::lazy_static;

use super::process::Process;

lazy_static! {
    pub static ref SCHEDULER: Scheduler = Scheduler::new();
}

#[derive(Debug)]
pub struct Scheduler {
    pub processes: VecDeque<Process>,
}

impl Scheduler {
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
