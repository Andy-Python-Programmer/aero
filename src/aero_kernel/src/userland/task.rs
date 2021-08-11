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
use xmas_elf::ElfFile;

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::mem::paging::*;

use crate::arch::task::ArchTask;
use crate::fs::file_table::FileTable;

use intrusive_collections::{intrusive_adapter, LinkedListLink};

use super::vm::Vm;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TaskId(usize);

impl TaskId {
    #[inline]
    pub(super) const fn new(pid: usize) -> Self {
        Self(pid)
    }

    /// Allocates a new task ID.
    fn allocate() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

        Self::new(NEXT_PID.fetch_add(1, Ordering::AcqRel))
    }

    #[inline]
    pub fn as_usize(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskState {
    Runnable,
}

pub struct Task {
    arch_task: UnsafeCell<ArchTask>,
    task_id: TaskId,
    vm: Arc<Vm>,

    pub file_table: Arc<FileTable>,
    pub state: TaskState,

    pub(super) link: intrusive_collections::LinkedListLink,
}

impl Task {
    /// Creates a per-cpu idle task. An idle task is a special *kernel*
    /// which is executed when there are no runnable taskes in the scheduler's
    /// queue.
    #[inline]
    pub fn new_idle() -> Arc<Task> {
        Arc::new(Self {
            arch_task: UnsafeCell::new(ArchTask::new_idle()),
            file_table: Arc::new(FileTable::new()),
            task_id: TaskId::allocate(),
            vm: Arc::new(Vm::new()),
            state: TaskState::Runnable,

            link: Default::default(),
        })
    }

    /// Allocates a new kernel task pointing at the provided entry point address. This function
    /// is responsible for creating the kernel task and setting up the context switch stack itself.
    #[inline]
    pub fn new_kernel(entry_point: VirtAddr) -> Arc<Self> {
        Arc::new(Self {
            arch_task: UnsafeCell::new(ArchTask::new_kernel(entry_point)),
            task_id: TaskId::allocate(),
            file_table: Arc::new(FileTable::new()),
            vm: Arc::new(Vm::new()),
            state: TaskState::Runnable,

            link: Default::default(),
        })
    }

    pub fn fork(&self) -> Arc<Task> {
        let arch_task = UnsafeCell::new(
            self.arch_task_mut()
                .fork()
                .expect("failed to fork arch task"),
        );

        let this = Arc::new(Self {
            arch_task,
            task_id: TaskId::allocate(),
            file_table: self.file_table.clone(),
            vm: Arc::new(Vm::new()),
            state: self.state,
            link: Default::default(),
        });

        this.vm.fork_from(self.vm());
        this
    }

    #[inline]
    pub fn exec(&self, executable: &ElfFile) -> Result<(), MapToError<Size4KiB>> {
        let vm = self.vm();

        vm.clear();
        vm.load_bin(executable);

        self.arch_task_mut().exec(vm, executable)
    }

    #[inline]
    pub fn vm(&self) -> &Arc<Vm> {
        &self.vm
    }

    /// Returns a mutable reference to the inner [ArchTask] structure.
    #[inline]
    pub fn arch_task_mut(&self) -> &mut ArchTask {
        unsafe { &mut (*self.arch_task.get()) }
    }

    /// Returns the task ID that was allocated for this task.
    #[inline]
    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    #[inline]
    pub fn handle_page_fault(
        &self,
        accessed_address: VirtAddr,
        reason: PageFaultErrorCode,
    ) -> bool {
        self.vm.handle_page_fault(reason, accessed_address)
    }
}

// Create a new intrustive adapter for the [Task] struct as the tasks are stored as a linked
// list in the scheduler.
intrusive_collections::intrusive_adapter!(pub TaskAdapter = Arc<Task> : Task { link: LinkedListLink });
