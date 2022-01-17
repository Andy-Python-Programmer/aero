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
use crate::utils::sync::{BlockQueue, Mutex};

use crate::userland::signals::Signals;

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

use super::scheduler;
use super::signals::{SignalResult, TriggerResult};
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

struct Cwd {
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

struct Zombies {
    list: Mutex<LinkedList<SchedTaskAdapter>>,
    block: BlockQueue,
}

impl Zombies {
    fn new() -> Self {
        Self {
            list: Mutex::new(Default::default()),
            block: BlockQueue::new(),
        }
    }

    fn add_zombie(&self, zombie: Arc<Task>) {
        assert_eq!(zombie.link.is_linked(), false);
        assert_eq!(zombie.state(), TaskState::Zombie);

        let mut list = self.list.lock();

        log::debug!("making process a zombie: (pid={:?})", zombie.pid());

        list.push_back(zombie);
        self.block.notify_complete();
    }

    fn waitpid(&self, pid: usize, status: &mut u32) -> SignalResult<usize> {
        let mut captured = (TaskId(0), 0);

        self.block.block_on(&self.list, |l| {
            let mut cursor = l.front_mut();

            while let Some(t) = cursor.get() {
                if t.pid().as_usize() == pid {
                    captured = (t.pid(), t.exit_status());
                    cursor.remove();

                    return true;
                } else {
                    cursor.move_next();
                }
            }

            false
        })?;

        let (tid, st) = captured;

        // WIFEXITED: The child process has been terminated normally by
        // either calling sys_exit() or returning from the main function.
        *status = 0x200;
        // The lower 8-bits are used to store the exit status.
        *status |= st as u32 & 0xff;

        Ok(tid.as_usize())
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

    zombies: Zombies,

    sleep_duration: AtomicUsize,
    signals: Signals,

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
            zombies: Zombies::new(),

            arch_task: UnsafeCell::new(ArchTask::new_idle()),
            file_table: Arc::new(FileTable::new()),

            tid: pid.clone(),
            pid,

            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            link: Default::default(),
            clink: Default::default(),

            sleep_duration: AtomicUsize::new(0),
            exit_status: AtomicIsize::new(0),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            signals: Signals::new(),
            cwd: Cwd::new(),
        })
    }

    /// Allocates a new kernel task pointing at the provided entry point function.
    pub fn new_kernel(entry_point: fn(), enable_interrupts: bool) -> Arc<Self> {
        let pid = TaskId::allocate();

        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),
            zombies: Zombies::new(),

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

            sleep_duration: AtomicUsize::new(0),
            exit_status: AtomicIsize::new(0),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            signals: Signals::new(),
            cwd: Cwd::new(),
        })
    }

    fn make_child(&self, arch_task: UnsafeCell<ArchTask>) -> Arc<Task> {
        let pid = TaskId::allocate();

        let this = Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),
            zombies: Zombies::new(),

            arch_task,
            file_table: Arc::new(self.file_table.deep_clone()),
            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            link: Default::default(),
            clink: Default::default(),

            sleep_duration: AtomicUsize::new(0),
            exit_status: AtomicIsize::new(0),

            tid: pid.clone(),
            pid,

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            cwd: self.cwd.read().fork(),
            signals: Signals::new(),
        });

        self.add_child(this.clone());
        this.signals().copy_from(self.signals());

        this.vm.fork_from(self.vm());
        this.vm.log();
        this
    }

    pub fn signals(&self) -> &Signals {
        &self.signals
    }

    pub fn clone_process(&self, entry: usize, stack: usize) -> Arc<Task> {
        let arch_task = UnsafeCell::new(
            self.arch_task_mut()
                .clone_process(entry, stack)
                .expect("failed to fork arch task"),
        );

        self.make_child(arch_task)
    }

    pub fn fork(&self) -> Arc<Task> {
        let arch_task = UnsafeCell::new(
            self.arch_task_mut()
                .fork()
                .expect("failed to fork arch task"),
        );

        self.make_child(arch_task)
    }

    fn this(&self) -> Arc<Self> {
        self.sref.upgrade().unwrap()
    }

    fn set_parent(&self, parent: Option<Arc<Task>>) {
        *self.parent.lock() = parent;
    }

    fn remove_child(&self, child: &Task) {
        let mut children = self.children.lock();

        if child.clink.is_linked() {
            let mut cursor = unsafe { children.cursor_mut_from_ptr(child) };

            child.set_parent(None);
            cursor.remove();
        }
    }

    fn add_child(&self, child: Arc<Task>) {
        let mut children = self.children.lock();

        child.set_parent(Some(self.this()));
        children.push_back(child);
    }

    fn exit_status(&self) -> isize {
        self.exit_status.load(Ordering::SeqCst)
    }

    pub fn set_sleep_duration(&self, duration: usize) {
        self.sleep_duration.store(duration, Ordering::SeqCst);
    }

    pub fn load_sleep_duration(&self) -> usize {
        self.sleep_duration.load(Ordering::SeqCst)
    }

    pub fn waitpid(&self, pid: usize, status: &mut u32) -> SignalResult<usize> {
        self.zombies.waitpid(pid, status)
    }

    pub fn exec(
        &self,
        executable: DirCacheItem,

        argv: Option<ExecArgs>,
        envv: Option<ExecArgs>,
    ) -> Result<(), MapToError<Size4KiB>> {
        let vm = self.vm();
        vm.clear();

        // Clear the signals that are pending for this task on exec.
        self.signals().clear();

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

    /// Returns the PID ID that was allocated for this task.
    pub fn pid(&self) -> TaskId {
        self.pid
    }

    pub fn tid(&self) -> TaskId {
        self.tid
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

    fn get_parent(&self) -> Option<Arc<Task>> {
        let parent = self.parent.lock();
        parent.clone()
    }

    pub fn wake_up(&self) {
        scheduler::get_scheduler().inner.wake_up(self.this())
    }

    pub fn is_process_leader(&self) -> bool {
        self.tid() == self.pid()
    }

    pub fn process_leader(&self) -> Arc<Task> {
        if self.is_process_leader() {
            self.this()
        } else {
            let parent = self.get_parent().unwrap();

            assert!(parent.is_process_leader());
            parent
        }
    }

    pub fn signal(&self, signal: usize) -> bool {
        match self.signals().trigger(signal, false) {
            TriggerResult::Triggered => {
                self.wake_up();
                true
            }

            TriggerResult::Ignored => false,

            TriggerResult::Blocked => {
                // Find other thread in process to notify
                let process_leader = self.process_leader();

                if !process_leader.signals().is_blocked(signal) {
                    process_leader.wake_up();

                    return true;
                }

                for c in process_leader
                    .children
                    .lock()
                    .iter()
                    .filter(|t| t.pid() == self.pid())
                {
                    if !c.signals().is_blocked(signal) {
                        c.wake_up();

                        return true;
                    }
                }

                false
            }
        }
    }

    pub(super) fn into_zombie(&self) {
        // TODO: Deallocate the arch task's resources.

        if let Some(parent) = self.get_parent() {
            parent.remove_child(self);
            parent.zombies.add_zombie(self.this());

            // TODO: If the parent process is the process leader then
            // signal SIGCHLD to the parent process.
        }
    }
}

unsafe impl Sync for Task {}

// Create a new intrustive adapter for the [Task] struct as the tasks are stored as a linked
// list in the scheduler.
intrusive_collections::intrusive_adapter!(pub SchedTaskAdapter = Arc<Task> : Task { link: LinkedListLink });
intrusive_collections::intrusive_adapter!(pub TaskAdapter = Arc<Task> : Task { clink: LinkedListLink });
