/*
 * Copyright (C) 2021 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

#[cfg(feature = "round-robin")]
pub mod round_robin;

#[cfg(feature = "round-robin")]
pub use round_robin::{exit_current_process, reschedule};

use alloc::{collections::BTreeMap, sync::Arc};

use spin::mutex::spin::{SpinMutex, SpinMutexGuard};
use spin::Once;

use crate::utils::Downcastable;

use self::round_robin::RoundRobin;
use super::process::{Process, ProcessId};

static SCHEDULER: Once<SpinMutex<Scheduler>> = Once::new();
static PROCESS_CONTAINER: ProcessContainer = ProcessContainer::new_uninit();

/// Scheduler interface for each scheduling algorithm. The struct implementing
/// this trait has to implement [Send], [Sync] and [Downcastable].
pub trait SchedulerInterface: Send + Sync + Downcastable {
    /// Register the provided process into the task scheduler queue.
    fn register_process(&self, process_id: ProcessId);
}

/// Container or a transparent struct containing a hashmap of all of the processes
/// in the scheduler's queue protected by mutex. The hashmap has a key
/// of `ProcessId` and a value of a reference-counting pointer
/// to the process or task.
#[repr(transparent)]
struct ProcessContainer(SpinMutex<BTreeMap<ProcessId, Arc<SpinMutex<Process>>>>);

impl ProcessContainer {
    /// Creates a new process container with no processes by default.
    #[inline]
    const fn new_uninit() -> Self {
        Self(SpinMutex::new(BTreeMap::new()))
    }

    /// Registers the provided `process` in the process container.
    #[inline]
    fn register_process(&self, process_id: ProcessId, process: Arc<SpinMutex<Process>>) {
        self.0.lock().insert(process_id, process);
    }

    #[inline]
    fn find_by_id(&self, id: ProcessId) -> Option<Arc<SpinMutex<Process>>> {
        self.0.lock().get(&id).cloned()
    }
}

pub struct Scheduler {
    inner: Arc<dyn SchedulerInterface>,
}

impl Scheduler {
    /// Create a new scheduler with no active tasks by default.
    #[inline]
    fn new() -> Self {
        Self {
            #[cfg(feature = "round-robin")]
            inner: RoundRobin::new(),
        }
    }

    /// Registers the provided process in the schedulers queue.
    pub fn register_process(&self, process: Arc<SpinMutex<Process>>) {
        let process_id = process.lock().process_id;

        self.inner.register_process(process_id);
        PROCESS_CONTAINER.register_process(process_id, process);
    }
}

/// Get a reference to the active scheduler.
pub fn get_scheduler() -> SpinMutexGuard<'static, Scheduler> {
    SCHEDULER
        .get()
        .expect("Attempted to get the scheduler before it was initialized")
        .lock()
}

/// Initialize the scheduler.
#[inline]
pub fn init() {
    SCHEDULER.call_once(move || SpinMutex::new(Scheduler::new()));
}
