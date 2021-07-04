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

use alloc::sync::Arc;

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::mem::paging::VirtAddr;

use crate::arch::task::ArchTask;
use crate::fs::file_table::FileTable;

use intrusive_collections::{intrusive_adapter, LinkedListLink};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TaskId(usize);

impl TaskId {
    #[inline(always)]
    pub(super) const fn new(pid: usize) -> Self {
        Self(pid)
    }

    /// Allocates a new task ID.
    fn allocate() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

        Self::new(NEXT_PID.fetch_add(1, Ordering::AcqRel))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaskState {
    Running,
}

pub struct Task {
    arch_task: UnsafeCell<ArchTask>,
    pub(super) task_id: TaskId,

    pub file_table: FileTable,
    pub state: TaskState,

    pub(super) link: intrusive_collections::LinkedListLink,
}

impl Task {
    /// Creates a per-cpu idle task. An idle task is a special *kernel*
    /// which is executed when there are no runnable taskes in the scheduler's
    /// queue.
    pub fn new_idle() -> Arc<Task> {
        Arc::new(Self {
            arch_task: UnsafeCell::new(ArchTask::new_idle()),
            file_table: FileTable::new(),
            task_id: TaskId::allocate(),
            state: TaskState::Running,

            link: Default::default(),
        })
    }

    /// Allocates a new kernel task pointing at the provided entry point address. This function
    /// is responsible for creating the kernel task and setting up the context switch stack itself.
    pub fn new_kernel(entry_point: VirtAddr) -> Arc<Self> {
        Arc::new(Self {
            arch_task: UnsafeCell::new(ArchTask::new_kernel(entry_point)),
            task_id: TaskId::allocate(),
            file_table: FileTable::new(),
            state: TaskState::Running,

            link: Default::default(),
        })
    }

    #[inline]
    pub fn arch_task_mut(&self) -> &mut ArchTask {
        unsafe { &mut (*self.arch_task.get()) }
    }

    #[inline]
    pub fn arch_task_ref(&self) -> &ArchTask {
        unsafe { &(*self.arch_task.get()) }
    }
}

intrusive_collections::intrusive_adapter!(pub TaskAdapter = Arc<Task> : Task { link: LinkedListLink });
