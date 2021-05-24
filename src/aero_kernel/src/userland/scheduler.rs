/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use alloc::sync::Arc;
use spin::{Mutex, Once};

use hashbrown::HashMap;

use super::process::{Process, ProcessId};

static SCHEDULER: Once<Scheduler> = Once::new();

/// Container or a transparent struct containing a hashmap of all of the processes
/// in the scheduler's queue protected by mutex. The hashmap has a key
/// of `ProcessId` and a value of a reference-counting pointer
/// to the process or task.
#[repr(transparent)]
struct ProcessContainer(Mutex<HashMap<ProcessId, Arc<Process>>>);

impl ProcessContainer {
    /// Creates a new task container with no tasks by default.
    #[inline]
    fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    /// Registers the provided `process` in the process container.
    #[inline]
    fn register_process(&self, process: Arc<Process>) {
        self.0.lock().insert(process.process_id, process);
    }

    fn find_process_by_id(&self, id: ProcessId) -> Option<Arc<Process>> {
        self.0.lock().get(&id).cloned()
    }
}

pub struct Scheduler {
    processes: ProcessContainer,
}

impl Scheduler {
    /// Create a new scheduler with no active tasks by default.
    #[inline]
    fn new() -> Self {
        Self {
            processes: ProcessContainer::new(),
        }
    }

    pub fn push(&self, process: Arc<Process>) {
        let context = process.get_context_ref();

        let instruction_ptr = context.get_instruction_ptr();
        let stack_top = context.get_stack_top();
        let rflags = context.rflags;

        self.processes.register_process(process);

        unsafe {
            super::jump_userland(stack_top, instruction_ptr, rflags);
        }
    }

    pub fn active_task_ref(&self) -> Option<Arc<Process>> {
        /*
         * FIXME(Andy-Python-Programmer): Support multiple processes. Currently
         * we can only run one which is royal pain :D
         */

        self.processes.find_process_by_id(ProcessId::new(1))
    }
}

/// Get a mutable reference to the active scheduler.
pub fn get_scheduler() -> &'static Scheduler {
    SCHEDULER
        .get()
        .expect("Attempted to get the scheduler before it was initialized")
}

pub fn reschedule() -> bool {
    true
}

/// Initialize the scheduler.
pub fn init() {
    SCHEDULER.call_once(move || Scheduler::new());
}
