/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
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

use alloc::sync::Arc;

use crate::utils::sync::Mutex;
use crate::{fs::cache::DirCacheItem, syscall::ExecArgs};

use spin::Once;

use crate::utils::Downcastable;

use self::round_robin::RoundRobin;
use super::signals::SignalResult;
use super::task::{Task, TaskId};

static SCHEDULER: Once<Scheduler> = Once::new();

/// Scheduler interface for each scheduling algorithm. The struct implementing
/// this trait has to implement [Send], [Sync] and [Downcastable].
pub trait SchedulerInterface: Send + Sync + Downcastable {
    /// Register the provided task into the task scheduler queue.
    fn register_task(&self, task: Arc<Task>);

    /// Get a reference-counting pointer to the current task.
    fn current_task(&self) -> Arc<Task>;

    fn init(&self);
    fn wake_up(&self, task: Arc<Task>);

    fn await_io(&self) -> SignalResult<()>;
    fn sleep(&self, duration: Option<usize>) -> SignalResult<()>;

    /// Yields execution to another task.
    fn preempt(&self);

    /// Exits the current task.
    fn exit(&self, status: isize) -> !;
}

/// Container or a transparent struct containing a hashmap of all of the taskes
/// in the scheduler's queue protected by mutex. The hashmap has a key
/// of `ProcessId` and a value of a reference-counting pointer
/// to the task or task.
#[repr(transparent)]
struct TaskContainer(Mutex<hashbrown::HashMap<TaskId, Arc<Task>>>);

impl TaskContainer {
    /// Creates a new task container with no taskes by default.
    #[inline]
    fn new() -> Self {
        Self(Mutex::new(hashbrown::HashMap::new()))
    }

    /// Registers the provided `task` in the task container.
    #[inline]
    fn register_task(&self, task_id: TaskId, task: Arc<Task>) {
        self.0.lock().insert(task_id, task);
    }
}

unsafe impl Send for TaskContainer {}
unsafe impl Sync for TaskContainer {}

pub struct Scheduler {
    tasks: TaskContainer,
    pub inner: Arc<dyn SchedulerInterface>,
}

impl Scheduler {
    /// Create a new scheduler with no active tasks by default.
    #[inline]
    fn new() -> Self {
        Self {
            tasks: TaskContainer::new(),

            #[cfg(feature = "round-robin")]
            inner: RoundRobin::new(),
        }
    }

    /// Registers the provided task in the schedulers queue.
    pub fn register_task(&self, task: Arc<Task>) {
        self.tasks.register_task(task.pid(), task.clone());
        self.inner.register_task(task.clone());
    }

    #[inline]
    pub fn exec(&self, executable: DirCacheItem, argv: Option<ExecArgs>, envv: Option<ExecArgs>) {
        self.inner
            .current_task()
            .exec(executable, argv, envv)
            .unwrap();
    }

    /// Get the current task
    #[inline]
    pub fn current_task(&self) -> Arc<Task> {
        self.inner.current_task()
    }

    /// Lookup a task by ID
    #[inline]
    pub fn find_task(&self, task_id: TaskId) -> Option<Arc<Task>> {
        self.tasks.0.lock().get(&task_id).map(|task| task.clone())
    }
}

/// Get a reference to the active scheduler.
pub fn get_scheduler() -> &'static Scheduler {
    SCHEDULER
        .get()
        .expect("Attempted to get the scheduler before it was initialized")
}

/// Returns true if the task scheduler has been initiaized.
pub fn is_initialized() -> bool {
    SCHEDULER.get().is_some()
}

/// Initialize the scheduler.
#[inline]
pub fn init() {
    SCHEDULER.call_once(|| Scheduler::new()).inner.init();
}
