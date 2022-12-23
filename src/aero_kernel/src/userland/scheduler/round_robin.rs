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

use alloc::sync::Arc;

use intrusive_collections::LinkedList;

use crate::arch;
use crate::userland::signals::{SignalError, SignalResult};
use crate::userland::task::{SchedTaskAdapter, Task, TaskState};

use crate::utils::sync::IrqGuard;
use crate::utils::PerCpu;

use super::SchedulerInterface;

/// Scheduler queue containing a vector of all of the task of the enqueued
/// taskes.
struct TaskQueue {
    /// The kernel idle task is a special kind of task that is run when
    /// no taskes in the scheduler's queue are available to execute. The idle task
    /// is to be created for each CPU.
    idle_task: Arc<Task>,
    preempt_task: Arc<Task>,
    current_task: Option<Arc<Task>>,

    runnable: LinkedList<SchedTaskAdapter>,
    dead: LinkedList<SchedTaskAdapter>,
    awaiting: LinkedList<SchedTaskAdapter>,
    deadline_awaiting: LinkedList<SchedTaskAdapter>,
}

impl TaskQueue {
    /// Creates a new task queue with no taskes by default.
    fn new() -> Self {
        Self {
            idle_task: Task::new_idle(),
            preempt_task: Task::new_kernel(preempter, false),
            current_task: None,

            runnable: LinkedList::new(SchedTaskAdapter::new()),
            dead: LinkedList::new(SchedTaskAdapter::new()),
            awaiting: LinkedList::new(SchedTaskAdapter::new()),
            deadline_awaiting: LinkedList::new(SchedTaskAdapter::new()),
        }
    }

    fn push_runnable(&mut self, task: Arc<Task>) {
        debug_assert_eq!(task.link.is_linked(), false); // Make sure the task is not already linked

        task.update_state(TaskState::Runnable);
        self.runnable.push_back(task);
    }

    fn push_dead(&mut self, task: Arc<Task>) {
        debug_assert_eq!(task.state(), TaskState::Runnable);
        debug_assert_eq!(task.link.is_linked(), false); // Make sure the task is not already linked

        self.dead.push_back(task);
    }

    fn push_deadline_awaiting(&mut self, task: Arc<Task>, duration: usize) {
        debug_assert_eq!(task.link.is_linked(), false); // Make sure the task is not already linked

        task.update_state(TaskState::AwaitingIo);
        task.set_sleep_duration(crate::arch::time::get_uptime_ticks() + duration);

        self.deadline_awaiting.push_back(task);
    }

    fn push_awaiting(&mut self, task: Arc<Task>) {
        debug_assert_eq!(task.link.is_linked(), false); // Make sure the task is not already linked

        task.update_state(TaskState::AwaitingIo);
        self.awaiting.push_back(task);
    }
}

/// Round Robin is the simplest algorithm for a preemptive scheduler. When the
/// system timer fires, the next task in the queue is switched to, and the
/// preempted task is put back into the queue.
///
/// ## Notes
/// * <https://en.wikipedia.org/wiki/Round-robin_scheduling>
pub struct RoundRobin {
    /// The per-cpu scheduler queues.
    queue: PerCpu<TaskQueue>,
}

impl RoundRobin {
    /// Creates a new instance of the round robin scheduler and return a
    /// reference-counting pointer to itself.
    pub fn new() -> Arc<Self> {
        let this = Arc::new(Self {
            queue: PerCpu::new(|| TaskQueue::new()),
        });

        this
    }

    fn sweep_dead(&self) {
        let _guard = IrqGuard::new();
        let queue = self.queue.get_mut();

        if let Some(task) = queue.dead.pop_front() {
            task.update_state(TaskState::Zombie);
            task.into_zombie();
        }
    }

    fn schedule_check_deadline(&self) {
        let _guard = IrqGuard::new();
        let queue = self.queue.get_mut();

        let time = crate::arch::time::get_uptime_ticks();

        let mut cursor = queue.deadline_awaiting.front_mut();

        while let Some(task) = cursor.get() {
            if task.load_sleep_duration() <= time {
                let ptr = cursor.remove().unwrap();

                assert_eq!(ptr.link.is_linked(), false);

                ptr.update_state(TaskState::Runnable);
                ptr.set_sleep_duration(0);

                queue.runnable.push_back(ptr);
            } else {
                cursor.move_next();
            }
        }
    }

    fn schedule_next_task(&self) {
        let guard = IrqGuard::new();
        let queue = self.queue.get_mut();

        self.schedule_check_deadline();

        // Switch to the next runnable task in the runnable queue, and put
        // the preempted task back into the runnable queue.
        if let Some(task) = queue.runnable.pop_front() {
            if let Some(current_task) = queue.current_task.clone() {
                if !current_task.link.is_linked() && current_task.pid() != task.pid() {
                    queue.push_runnable(current_task);
                }
            }

            queue.current_task = Some(task.clone());
            core::mem::drop(guard);
            arch::task::arch_task_spinup(queue.preempt_task.arch_task_mut(), task.arch_task());
        } else {
            if let Some(current) = queue.current_task.as_ref() {
                if current.state() == TaskState::Runnable {
                    core::mem::drop(guard);
                    arch::task::arch_task_spinup(
                        queue.preempt_task.arch_task_mut(),
                        current.arch_task(),
                    );
                }
            } else {
                queue.current_task = None;
                core::mem::drop(guard);
                arch::task::arch_task_spinup(
                    queue.preempt_task.arch_task_mut(),
                    queue.idle_task.arch_task(),
                );
            }
        }
    }
}

impl SchedulerInterface for RoundRobin {
    fn register_task(&self, task: Arc<Task>) {
        let queue = self.queue.get_mut();

        queue.push_runnable(task);
    }

    fn current_task_optional(&self) -> Option<Arc<Task>> {
        self.queue.get().current_task.as_ref().cloned()
    }

    fn init(&self) {
        // Register the sweeper task in the scheduler's queue.
        super::get_scheduler().register_task(Task::new_kernel(sweeper, true));
    }

    fn wake_up(&self, task: Arc<Task>) {
        let _guard = IrqGuard::new();
        let queue = self.queue.get_mut();

        if task.state() == TaskState::AwaitingIo {
            let mut cursor = unsafe { queue.awaiting.cursor_mut_from_ptr(task.as_ref()) };

            if let Some(task) = cursor.remove() {
                queue.push_runnable(task);
            }
        } else {
            task.set_pending_io(true)
        }
    }

    fn sleep(&self, duration: Option<usize>) -> SignalResult<()> {
        let _guard = IrqGuard::new();
        let queue = self.queue.get_mut();

        let task = queue
            .current_task
            .as_ref()
            .expect("IDLE task should not await for anything")
            .clone();

        if task.has_pending_io() {
            task.set_pending_io(false);
            return Ok(());
        }

        if let Some(duration) = duration {
            queue.push_deadline_awaiting(task, duration);
        } else {
            queue.push_awaiting(task);
        }

        self.preempt();

        let task = queue
            .current_task
            .as_ref()
            .expect("IDLE task should not await for anything")
            .clone();

        if task.signals().has_pending() {
            Err(SignalError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn preempt(&self) {
        // We want to preempt under the following circumstances:
        //
        // 1. When a process switches from the running state to the waiting
        //    state.
        // 2. When the timer interrupt fires.
        // 3. When the process switches from the waiting state to the runnable state
        //    (for example, on completion of I/O operation).
        // 4. When the process is terminated.

        let guard = IrqGuard::new();
        let queue = self.queue.get();

        if let Some(current) = queue.current_task.as_ref() {
            core::mem::drop(guard);
            arch::task::arch_task_spinup(current.arch_task_mut(), queue.preempt_task.arch_task());
        } else {
            core::mem::drop(guard);
            arch::task::arch_task_spinup(
                queue.idle_task.arch_task_mut(),
                queue.preempt_task.arch_task(),
            );
        }
    }

    fn await_io(&self) -> SignalResult<()> {
        self.sleep(None)
    }

    fn exit(&self, status: isize) -> ! {
        let guard = IrqGuard::new();
        let queue = self.queue.get_mut();

        let current_task = queue
            .current_task
            .as_ref()
            .expect("attempted to exit current task before it was initialized")
            .clone();

        current_task
            .exit_status
            .store(status, core::sync::atomic::Ordering::SeqCst);

        queue.push_dead(current_task.clone());

        core::mem::drop(guard);
        self.preempt();

        unreachable!()
    }
}

unsafe impl Send for RoundRobin {}
unsafe impl Sync for RoundRobin {}

/// Special scheduler task which is responsible to terminate a child process
/// that has previously exited, thereby removing it from the process table. Until
/// the child process is sweeped, it will be listed in the process table as a zombie
/// or defunct process.
fn sweeper() {
    let scheduler_ref = super::get_scheduler()
        .inner
        .downcast_arc::<RoundRobin>()
        .unwrap();

    loop {
        scheduler_ref.sweep_dead();
    }
}

fn preempter() {
    let scheduler_ref = super::get_scheduler()
        .inner
        .downcast_arc::<RoundRobin>()
        .unwrap();

    loop {
        scheduler_ref.schedule_next_task();
    }
}
