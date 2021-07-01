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

use core::cell::Cell;
use core::mem;

use alloc::{collections::VecDeque, sync::Arc};
use spin::mutex::spin::SpinMutex;

use crate::userland::task::{Task, TaskId};
use crate::utils::{downcast, PerCpu};

use super::{SchedulerInterface, PROCESS_CONTAINER};

#[thread_local]
static mut CURRENT_PROCESS: Option<Arc<SpinMutex<Task>>> = None;

/// The kernel idle task is a special kind of task that is run when
/// no taskes in the scheduler's queue are avaliable to execute. The idle task
/// is to be created for each CPU.
#[thread_local]
static mut IDLE_PROCESS: Option<Arc<SpinMutex<Task>>> = None;

#[thread_local]
static mut HELD_LOCKS: Cell<Option<HeldLocks>> = Cell::new(None);

struct HeldLocks {
    head: Arc<SpinMutex<Task>>,
    tail: Arc<SpinMutex<Task>>,
}

/// Scheduler queue containing a vector of all of the task of the enqueued
/// taskes.
#[repr(transparent)]
struct ProcessQueue(SpinMutex<VecDeque<TaskId>>);

impl ProcessQueue {
    /// Creates a new task queue with no taskes by default.
    #[inline]
    fn new() -> Self {
        Self(SpinMutex::new(VecDeque::new()))
    }

    /// Registers the provided `task` in the task queue.
    #[inline]
    fn register_task(&self, task_id: TaskId) {
        self.0.lock().push_back(task_id);
    }

    fn front(&self) -> Option<TaskId> {
        self.0.lock().pop_front()
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
    queue: PerCpu<ProcessQueue>,
}

impl RoundRobin {
    /// Creates a new instance of the round robin scheduler and return a
    /// reference-counting pointer to itself. The task of this function
    /// is to initialize the per-cpu queues that the round robin scheduling
    /// algorithm requires.
    pub fn new() -> Arc<Self> {
        let idle_task = Task::new_idle();

        unsafe {
            CURRENT_PROCESS = Some(idle_task.clone());
            IDLE_PROCESS = Some(idle_task);
        }

        Arc::new(Self {
            queue: PerCpu::new(|| ProcessQueue::new()),
        })
    }
}

impl SchedulerInterface for RoundRobin {
    fn register_task(&self, task_id: TaskId) {
        let queue = self.queue.get();

        queue.register_task(task_id);
    }
}

unsafe impl Send for RoundRobin {}
unsafe impl Sync for RoundRobin {}

pub fn exit_current_task(_: usize) -> ! {
    loop {}
}

/// This function is responsible for releasing all of the locks on the current task and the
/// previous task. These taskes are locked by [reschedule] and this function is called after the
/// the context switch is done.
#[no_mangle]
unsafe extern "C" fn context_switch_finalize() {
    let held_locks = HELD_LOCKS
        .take()
        .expect("`HELD_LOCKS` static was not initialized");

    held_locks.head.force_unlock();
    held_locks.tail.force_unlock();

    log::info!("Unlocked context switch locks");
}

/// Yields execution to another task. The task of this function is to get the
/// task which is on the front of the task queue and jump to it. If no task are
/// avaviable for execution then the [IDLE_PROCESS] task is executed.
///
/// ## Overview
/// Instead of adding `reschedule` as a method in the [SchedulerInterface] trait we are making
/// this a normal function as in the trait case, the scheduler will be locked for a longer time. The
/// scheduler only needs lock protection for reserving the task id allocated.
pub fn reschedule() -> bool {
    let scheduler = super::get_scheduler();
    let round_robin: Option<Arc<RoundRobin>> = downcast(&scheduler.inner);
    let scheduler_ref = round_robin.expect("Failed to downcast the scheduler");

    let queue = scheduler_ref.queue.get();

    mem::drop(scheduler); // Unlock the scheduler

    let previous_task = unsafe {
        CURRENT_PROCESS
            .as_ref()
            .expect("`reschedule` was invoked with no active previous task")
            .clone()
    };

    let new_task = {
        match queue.front() {
            Some(new_task) => PROCESS_CONTAINER
                .find_by_id(new_task)
                .expect("Unknown task in queue"),

            None => unsafe {
                IDLE_PROCESS
                    .as_ref()
                    .expect("IDLE thread was not initialized")
                    .clone()
            },
        }
    };

    /*
     * Check if the pointer of the new task is the same as the new task. If thats
     * the case keep running the task and return out.
     */
    if Arc::ptr_eq(&new_task, &previous_task) {
        return false;
    }

    /*
     * Now that we have passed all of the checks, its time to run the actual task. We first
     * set the CURRENT_PROCESS static to the new task and then jump to ring 3. Leap of faith!
     */
    unsafe {
        CURRENT_PROCESS = Some(new_task.clone());
    }

    let mut previous_task_locked = previous_task.lock();
    let new_task_locked = new_task.lock();

    unsafe {
        HELD_LOCKS.set(Some(HeldLocks {
            head: previous_task.clone(),
            tail: new_task.clone(),
        }));

        let previous_arch = previous_task_locked.arch_task_mut();
        let new_arch = new_task_locked.arch_task_ref();

        crate::arch::task::arch_switch(previous_arch, new_arch);

        mem::forget(previous_task_locked);
        mem::forget(new_task_locked);

        mem::forget(previous_task);
        mem::forget(new_task);
    }

    true
}
