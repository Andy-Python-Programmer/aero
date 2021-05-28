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

use alloc::{collections::VecDeque, sync::Arc};
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
    /// Creates a new process container with no processes by default.
    #[inline]
    fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    /// Registers the provided `process` in the process container.
    #[inline]
    fn register_process(&self, process: Arc<Process>) {
        self.0.lock().insert(process.process_id, process);
    }

    fn find_by_id(&self, id: ProcessId) -> Option<Arc<Process>> {
        self.0.lock().get(&id).cloned()
    }
}

/// Scheduler queue containing a vector of all of the process id's of the enqueued
/// processes.
#[repr(transparent)]
struct ProcessQueue(Mutex<VecDeque<ProcessId>>);

impl ProcessQueue {
    /// Creates a new process queue with no processes by default.
    #[inline]
    fn new() -> Self {
        Self(Mutex::new(VecDeque::new()))
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

pub struct Scheduler {
    process_container: ProcessContainer,
    process_queue: ProcessQueue,
}

impl Scheduler {
    /// Create a new scheduler with no active tasks by default.
    #[inline]
    fn new() -> Self {
        Self {
            process_container: ProcessContainer::new(),
            process_queue: ProcessQueue::new(),
        }
    }

    pub fn register_process(&self, process: Arc<Process>) {
        self.process_queue.register_process(process.process_id);
        self.process_container.register_process(process);
    }

    pub fn reschedule(&self) -> bool {
        if let Some(process_id) = self.process_queue.front() {
            let process = self
                .process_container
                .find_by_id(process_id)
                .expect("Process not found in the process container");

            let context = process.get_context_ref();

            unsafe {
                super::jump_userland(
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

    pub fn active_task_ref(&self) -> Option<Arc<Process>> {
        /*
         * FIXME(Andy-Python-Programmer): Support multiple processes. Currently
         * we can only run one which is royal pain :D
         */

        self.process_container.find_by_id(ProcessId::new(1))
    }
}

/// Get a reference to the active scheduler.
pub fn get_scheduler() -> &'static Scheduler {
    SCHEDULER
        .get()
        .expect("Attempted to get the scheduler before it was initialized")
}

/// Initialize the scheduler.
pub fn init() {
    SCHEDULER.call_once(move || Scheduler::new());
}
