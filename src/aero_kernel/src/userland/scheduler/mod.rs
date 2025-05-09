// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

#[cfg(feature = "round-robin")]
pub mod round_robin;

use core::ops;

use alloc::sync::Arc;

use crate::arch::interrupts::{self, InterruptStack};
use crate::fs::cache::DirCacheItem;
use crate::syscall::ExecArgs;
use crate::utils::sync::Mutex;

use spin::Once;

use self::round_robin::RoundRobin;
use super::signals::SignalResult;
use super::task::sessions::SESSIONS;
use super::task::{Task, TaskId};

static SCHEDULER: Once<Scheduler> = Once::new();

#[downcastable]
pub trait SchedulerInterface: Send + Sync {
    /// Register the provided task into the task scheduler queue.
    fn register_task(&self, task: Arc<Task>);

    fn current_task(&self) -> Arc<Task> {
        self.current_task_optional()
            .expect("current_task: current task not found")
    }

    fn current_task_optional(&self) -> Option<Arc<Task>>;

    fn init(&self);
    fn wake_up(&self, task: Arc<Task>);

    fn await_io(&self) -> SignalResult<()>;
    fn sleep(&self, duration: Option<usize>) -> SignalResult<()>;

    /// Yields execution to another task.
    fn preempt(&self);

    /// Exits the current task.
    fn exit(&self, status: ExitStatus) -> !;
}

struct TaskContainer(Mutex<hashbrown::HashMap<TaskId, Arc<Task>>>);

impl TaskContainer {
    fn new() -> Self {
        Self(Mutex::new(hashbrown::HashMap::new()))
    }

    fn register_task(&self, task_id: TaskId, task: Arc<Task>) {
        self.0.lock().insert(task_id, task);
    }

    fn remove_task(&self, task: &Task) {
        self.0.lock().remove(&task.pid());
    }
}

unsafe impl Send for TaskContainer {}
unsafe impl Sync for TaskContainer {}

#[derive(Debug, Clone)]
pub enum ExitStatus {
    Normal(isize),
    Signal(usize),
}

pub struct Scheduler {
    tasks: TaskContainer,
    inner: Arc<dyn SchedulerInterface>,
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
        SESSIONS.register_task(task.clone());
        self.inner.register_task(task);
    }

    #[inline]
    pub fn exec(&self, executable: &DirCacheItem, argv: Option<ExecArgs>, envv: Option<ExecArgs>) {
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

    pub fn exit(&self, status: ExitStatus) -> ! {
        let current_task = self.inner.current_task();
        SESSIONS.remove_task(&current_task);
        self.tasks.remove_task(&current_task);
        self.inner.exit(status)
    }

    pub fn log_ptable(&self) {
        self.tasks.0.lock().iter().for_each(|(pid, task)| {
            let path: String = task
                .path()
                .map(|path| path.into())
                .unwrap_or("<unknown>".into());

            log::info!(
                "task(pid={pid:?}, path={:?}, state={:?})",
                path,
                task.state()
            )
        });
    }

    pub fn for_each_task<F: FnMut(&Arc<Task>)>(&self, mut f: F) {
        self.tasks.0.lock().iter().for_each(|(_, task)| f(task));
    }

    /// Lookup a task by ID
    #[inline]
    pub fn find_task(&self, task_id: TaskId) -> Option<Arc<Task>> {
        self.tasks.0.lock().get(&task_id).cloned()
    }
}

impl ops::Deref for Scheduler {
    type Target = dyn SchedulerInterface;

    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

/// Get a reference to the active scheduler.
pub fn get_scheduler() -> &'static Scheduler {
    SCHEDULER
        .get()
        .expect("Attempted to get the scheduler before it was initialized")
}

#[inline]
pub fn current_thread() -> Arc<Task> {
    get_scheduler().current_task()
}

/// Returns true if the task scheduler has been initiaized.
pub fn is_initialized() -> bool {
    SCHEDULER.get().is_some()
}

static SCHEDULER_VECTOR: Once<u8> = Once::new();
const SCHEDULER_TIMER_US: usize = 5000;

fn scheduler_irq_handler(_stack: &mut InterruptStack) {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::apic::get_local_apic()
            .timer_oneshot(*SCHEDULER_VECTOR.get().unwrap(), SCHEDULER_TIMER_US);

        crate::arch::interrupts::INTERRUPT_CONTROLLER.eoi();
    }

    self::get_scheduler().inner.preempt();
}

/// Initialize the scheduler and set up the scheduler interrupt.
pub fn init() {
    SCHEDULER.call_once(Scheduler::new).inner.init();

    let scheduler_vector = interrupts::allocate_vector();
    interrupts::register_handler(scheduler_vector, scheduler_irq_handler);

    #[cfg(target_arch = "x86_64")]
    crate::arch::apic::get_local_apic().timer_oneshot(scheduler_vector, SCHEDULER_TIMER_US);
    SCHEDULER_VECTOR.call_once(|| scheduler_vector);
}
