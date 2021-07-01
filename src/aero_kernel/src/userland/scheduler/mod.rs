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
pub use round_robin::{exit_current_task, reschedule};

use alloc::{collections::BTreeMap, sync::Arc};

use spin::mutex::spin::{SpinMutex, SpinMutexGuard};
use spin::Once;

use crate::utils::Downcastable;

use self::round_robin::RoundRobin;
use super::task::{Task, TaskId};

static SCHEDULER: Once<SpinMutex<Scheduler>> = Once::new();
static PROCESS_CONTAINER: ProcessContainer = ProcessContainer::new_uninit();

/// Scheduler interface for each scheduling algorithm. The struct implementing
/// this trait has to implement [Send], [Sync] and [Downcastable].
pub trait SchedulerInterface: Send + Sync + Downcastable {
    /// Register the provided task into the task scheduler queue.
    fn register_task(&self, task_id: TaskId);
}

/// Container or a transparent struct containing a hashmap of all of the taskes
/// in the scheduler's queue protected by mutex. The hashmap has a key
/// of `ProcessId` and a value of a reference-counting pointer
/// to the task or task.
#[repr(transparent)]
struct ProcessContainer(SpinMutex<BTreeMap<TaskId, Arc<SpinMutex<Task>>>>);

impl ProcessContainer {
    /// Creates a new task container with no taskes by default.
    #[inline]
    const fn new_uninit() -> Self {
        Self(SpinMutex::new(BTreeMap::new()))
    }

    /// Registers the provided `task` in the task container.
    #[inline]
    fn register_task(&self, task_id: TaskId, task: Arc<SpinMutex<Task>>) {
        self.0.lock().insert(task_id, task);
    }

    #[inline]
    fn find_by_id(&self, id: TaskId) -> Option<Arc<SpinMutex<Task>>> {
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

    /// Registers the provided task in the schedulers queue.
    pub fn register_task(&self, task: Arc<SpinMutex<Task>>) {
        let task_id = task.lock().task_id;

        self.inner.register_task(task_id);
        PROCESS_CONTAINER.register_task(task_id, task);
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
