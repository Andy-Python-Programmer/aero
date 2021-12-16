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

use alloc::string::String;
use alloc::sync::{Arc, Weak};

use spin::RwLock;

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicIsize, AtomicU8, AtomicUsize, Ordering};

use crate::fs::cache::{DirCacheImpl, DirCacheItem};
use crate::fs::{self, FileSystem};
use crate::mem::paging::*;

use crate::arch::task::ArchTask;
use crate::fs::file_table::FileTable;
use crate::syscall::ExecArgs;
use crate::utils::sync::Mutex;

use intrusive_collections::{intrusive_adapter, LinkedListLink};

use super::vm::Vm;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TaskId(usize);

impl TaskId {
    pub(super) const fn new(pid: usize) -> Self {
        Self(pid)
    }

    /// Allocates a new task ID.
    fn allocate() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

        Self::new(NEXT_PID.fetch_add(1, Ordering::AcqRel))
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskState {
    Runnable,
    Zombie,
    AwaitingIo,
}

impl From<u8> for TaskState {
    fn from(x: u8) -> Self {
        match x {
            0 => TaskState::Runnable,
            1 => TaskState::Zombie,
            2 => TaskState::AwaitingIo,
            _ => panic!("invalid task state"),
        }
    }
}

pub struct Cwd {
    inode: DirCacheItem,
    filesystem: Arc<dyn FileSystem>,
}

impl Cwd {
    fn new() -> RwLock<Self> {
        let root = fs::root_dir().clone();
        let fs = root.inode().weak_filesystem().unwrap().upgrade().unwrap();

        RwLock::new(Self {
            inode: root,
            filesystem: fs,
        })
    }

    fn fork(&self) -> RwLock<Self> {
        RwLock::new(Self {
            inode: self.inode.clone(),
            filesystem: self.filesystem.clone(),
        })
    }
}

pub struct Task {
    sref: Weak<Task>,

    arch_task: UnsafeCell<ArchTask>,
    state: AtomicU8,

    // Note: Aero implementes the threads and as standard processes. This
    // means that when a new process is created its TID == PID and when a new
    // thread is created then the PID of the thread will be the process leader's
    // PID and the TID will be uniquely generated.
    pid: TaskId,
    tid: TaskId,

    parent: Mutex<Option<Arc<Task>>>,
    children: Mutex<intrusive_collections::LinkedList<TaskAdapter>>,

    pub(super) link: intrusive_collections::LinkedListLink,
    pub(super) clink: intrusive_collections::LinkedListLink,

    pub vm: Arc<Vm>,
    pub file_table: Arc<FileTable>,

    cwd: RwLock<Cwd>,

    pub(super) exit_status: AtomicIsize,
}

impl Task {
    /// Creates a per-cpu idle task. An idle task is a special *kernel* process
    /// which is executed when there are no runnable taskes in the scheduler's
    /// queue.
    pub fn new_idle() -> Arc<Task> {
        let pid = TaskId::allocate();

        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            arch_task: UnsafeCell::new(ArchTask::new_idle()),
            file_table: Arc::new(FileTable::new()),

            tid: pid.clone(),
            pid,

            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            link: Default::default(),
            clink: Default::default(),

            exit_status: AtomicIsize::new(0),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            cwd: Cwd::new(),
        })
    }

    /// Allocates a new kernel task pointing at the provided entry point function.
    pub fn new_kernel(entry_point: fn(), enable_interrupts: bool) -> Arc<Self> {
        let pid = TaskId::allocate();

        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            arch_task: UnsafeCell::new(ArchTask::new_kernel(
                VirtAddr::new(entry_point as u64),
                enable_interrupts,
            )),
            file_table: Arc::new(FileTable::new()),
            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            tid: pid.clone(),
            pid,

            link: Default::default(),
            clink: Default::default(),

            exit_status: AtomicIsize::new(0),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            cwd: Cwd::new(),
        })
    }

    pub fn fork(&self) -> Arc<Task> {
        let arch_task = UnsafeCell::new(
            self.arch_task_mut()
                .fork()
                .expect("failed to fork arch task"),
        );

        let pid = TaskId::allocate();

        let this = Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            arch_task,
            file_table: self.file_table.clone(),
            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            link: Default::default(),
            clink: Default::default(),

            exit_status: AtomicIsize::new(0),

            tid: pid.clone(),
            pid,

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            cwd: self.cwd.read().fork(),
        });

        self.add_child(this.clone());

        this.vm.fork_from(self.vm());
        this.vm.log();
        this
    }

    fn this(&self) -> Arc<Self> {
        self.sref.upgrade().unwrap()
    }

    fn set_parent(&self, parent: Option<Arc<Task>>) {
        *self.parent.lock() = parent;
    }

    fn add_child(&self, child: Arc<Task>) {
        let mut children = self.children.lock();

        child.set_parent(Some(self.this()));
        children.push_back(child);
    }

    pub fn exec(
        &self,
        executable: DirCacheItem,

        argv: Option<ExecArgs>,
        envv: Option<ExecArgs>,
    ) -> Result<(), MapToError<Size4KiB>> {
        let vm = self.vm();

        vm.clear();
        let loaded_binary = vm.load_bin(executable);

        self.arch_task_mut().exec(vm, loaded_binary, argv, envv)
    }

    pub fn vm(&self) -> &Arc<Vm> {
        &self.vm
    }

    /// Returns a immutable reference to the inner [ArchTask] structure.
    pub fn arch_task(&self) -> &ArchTask {
        unsafe { &(*self.arch_task.get()) }
    }

    /// Returns a mutable reference to the inner [ArchTask] structure.
    pub fn arch_task_mut(&self) -> &mut ArchTask {
        unsafe { &mut (*self.arch_task.get()) }
    }

    pub(super) fn update_state(&self, state: TaskState) {
        self.state.store(state as _, Ordering::SeqCst);
    }

    pub fn state(&self) -> TaskState {
        self.state.load(Ordering::SeqCst).into()
    }

    /// Returns the task ID that was allocated for this task.
    pub fn task_id(&self) -> TaskId {
        self.pid
    }

    pub fn get_cwd_dirent(&self) -> DirCacheItem {
        self.cwd.read().inode.clone()
    }

    pub fn get_cwd(&self) -> String {
        self.cwd.read().inode.absolute_path_str()
    }

    pub fn set_cwd(&self, cwd: DirCacheItem) {
        let filesystem = cwd.inode().weak_filesystem().unwrap().upgrade().unwrap();

        self.cwd.write().inode = cwd;
        self.cwd.write().filesystem = filesystem;
    }

    pub(super) fn into_zombie(&self) {}
}

unsafe impl Sync for Task {}

// Create a new intrustive adapter for the [Task] struct as the tasks are stored as a linked
// list in the scheduler.
intrusive_collections::intrusive_adapter!(pub SchedTaskAdapter = Arc<Task> : Task { link: LinkedListLink });
intrusive_collections::intrusive_adapter!(pub TaskAdapter = Arc<Task> : Task { clink: LinkedListLink });
