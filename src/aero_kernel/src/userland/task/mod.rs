// Copyright (C) 2021-2023 The Aero Project Developers.
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

pub mod sessions;

use aero_syscall::WaitPidFlags;
use alloc::sync::{Arc, Weak};

use spin::{Once, RwLock};

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

use crate::fs::cache::{DirCacheImpl, DirCacheItem};
use crate::fs::{self, FileSystem};
use crate::mem::paging::*;

use crate::arch::task::ArchTask;
use crate::fs::file_table::FileTable;
use crate::syscall::{ExecArgs, MessageQueue};
use crate::utils::sync::{Mutex, WaitQueue};

use crate::userland::signals::Signals;

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

use super::scheduler::{self, ExitStatus};
use super::signals::{SignalResult, TriggerResult};
use super::terminal::TerminalDevice;
use super::vm::Vm;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TaskId(usize);

impl TaskId {
    pub const fn new(pid: usize) -> Self {
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
    fn new() -> Self {
        let root = fs::root_dir().clone();
        let fs = root.inode().weak_filesystem().unwrap().upgrade().unwrap();

        Self {
            inode: root,
            filesystem: fs,
        }
    }

    fn fork(&self) -> Self {
        Self {
            inode: self.inode.clone(),
            filesystem: self.filesystem.clone(),
        }
    }
}

struct Zombies {
    list: Mutex<LinkedList<SchedTaskAdapter>>,
    block: WaitQueue,
}

impl Zombies {
    fn new() -> Self {
        Self {
            list: Mutex::new(Default::default()),
            block: WaitQueue::new(),
        }
    }

    fn add_zombie(&self, zombie: Arc<Task>) {
        assert!(!zombie.link.is_linked());
        assert_eq!(zombie.state(), TaskState::Zombie);

        let mut list = self.list.lock();

        log::debug!("making process a zombie: (pid={:?})", zombie.pid());

        list.push_back(zombie);
        self.block.notify_all();
    }

    fn waitpid(
        &self,
        pids: &[usize],
        status: &mut u32,
        flags: WaitPidFlags,
    ) -> SignalResult<usize> {
        let mut captured = None;

        self.block.block_on(&self.list, |l| {
            let mut cursor = l.front_mut();

            while let Some(t) = cursor.get() {
                for pid in pids {
                    if t.pid().as_usize() == *pid {
                        captured = Some((t.pid(), t.exit_status().clone()));
                        cursor.remove();

                        return true;
                    }
                }

                cursor.move_next();
            }

            if flags.contains(WaitPidFlags::WNOHANG) {
                return true;
            }

            false
        })?;

        if let Some((tid, exit_status)) = captured {
            // mlibc/abis/linux/wait.h (`W_EXITCODE`)
            match exit_status {
                ExitStatus::Normal(code) => {
                    *status = (code as u32) << 8;
                }

                ExitStatus::Signal(signal) => {
                    *status = signal as u32;
                }
            }

            Ok(tid.as_usize())
        } else {
            // If `WNOHANG` was specified in flags and there were no children in a waitable
            // state, then waipid() returns 0 immediately.
            *status = 0;
            Ok(0)
        }
    }
}

pub struct Task {
    sref: Weak<Task>,

    arch_task: UnsafeCell<ArchTask>,
    state: AtomicU8,

    pid: TaskId,
    tid: TaskId,

    sid: AtomicUsize,
    gid: AtomicUsize,

    parent: Mutex<Option<Arc<Task>>>,
    children: Mutex<intrusive_collections::LinkedList<TaskAdapter>>,

    zombies: Zombies,

    sleep_duration: AtomicUsize,
    signals: Signals,

    executable: Mutex<Option<DirCacheItem>>,
    pending_io: AtomicBool,

    pub(super) link: intrusive_collections::LinkedListLink,
    pub(super) clink: intrusive_collections::LinkedListLink,

    pub vm: Arc<Vm>,
    pub file_table: Arc<FileTable>,

    pub message_queue: MessageQueue,

    cwd: RwLock<Option<Cwd>>,

    pub(super) exit_status: Once<ExitStatus>,

    controlling_terminal: Mutex<Option<Arc<dyn TerminalDevice>>>,
    systrace: AtomicBool,
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

            message_queue: MessageQueue::new(),

            tid: pid,
            sid: AtomicUsize::new(pid.as_usize()),
            gid: AtomicUsize::new(pid.as_usize()),
            pid,

            executable: Mutex::new(None),

            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            link: Default::default(),
            clink: Default::default(),

            pending_io: AtomicBool::new(false),

            sleep_duration: AtomicUsize::new(0),
            exit_status: Once::new(),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            signals: Signals::new(),
            cwd: RwLock::new(None),

            systrace: AtomicBool::new(false),
            controlling_terminal: Mutex::new(None),
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
            message_queue: MessageQueue::new(),
            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            tid: pid,
            gid: AtomicUsize::new(pid.as_usize()),
            sid: AtomicUsize::new(pid.as_usize()),
            pid,

            link: Default::default(),
            clink: Default::default(),

            sleep_duration: AtomicUsize::new(0),
            exit_status: Once::new(),

            executable: Mutex::new(None),
            pending_io: AtomicBool::new(false),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            signals: Signals::new(),
            cwd: RwLock::new(None),

            systrace: AtomicBool::new(false),
            controlling_terminal: Mutex::new(None),
        })
    }

    fn make_child(&self, arch_task: UnsafeCell<ArchTask>) -> Arc<Task> {
        let pid = TaskId::allocate();

        let this = Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),
            zombies: Zombies::new(),

            arch_task,
            file_table: Arc::new(self.file_table.deep_clone()),
            message_queue: MessageQueue::new(),
            vm: Arc::new(Vm::new()),
            state: AtomicU8::new(TaskState::Runnable as _),

            link: Default::default(),
            clink: Default::default(),

            sleep_duration: AtomicUsize::new(0),
            exit_status: Once::new(),

            tid: pid,
            sid: AtomicUsize::new(self.session_id()),
            gid: AtomicUsize::new(self.group_id()),
            pid,

            executable: Mutex::new(self.executable.lock().clone()),
            pending_io: AtomicBool::new(false),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            cwd: RwLock::new(Some(self.cwd.read().as_ref().unwrap().fork())),
            signals: Signals::new(),

            systrace: AtomicBool::new(self.systrace()),
            controlling_terminal: Mutex::new(self.controlling_terminal.lock_irq().clone()),
        });

        self.add_child(this.clone());
        this.signals().copy_from(self.signals());

        this.vm.fork_from(self.vm());
        this
    }

    pub fn has_pending_io(&self) -> bool {
        self.pending_io.load(Ordering::SeqCst)
    }

    pub fn set_pending_io(&self, yes: bool) {
        self.pending_io.store(yes, Ordering::SeqCst)
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

        let pid = TaskId::allocate();

        let this = Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),
            zombies: Zombies::new(),

            arch_task,
            file_table: self.process_leader().file_table.clone(),
            message_queue: MessageQueue::new(),
            vm: self.process_leader().vm.clone(),
            state: AtomicU8::new(TaskState::Runnable as _),

            link: Default::default(),
            clink: Default::default(),

            sleep_duration: AtomicUsize::new(0),
            exit_status: Once::new(),

            tid: pid,
            sid: AtomicUsize::new(self.session_id()),
            gid: AtomicUsize::new(self.group_id()),
            pid,

            executable: Mutex::new(self.executable.lock().clone()),
            pending_io: AtomicBool::new(false),

            children: Mutex::new(Default::default()),
            parent: Mutex::new(None),

            cwd: RwLock::new(Some(self.cwd.read().as_ref().unwrap().fork())),
            signals: Signals::new(),

            systrace: AtomicBool::new(self.process_leader().systrace()),
            controlling_terminal: Mutex::new(
                self.process_leader()
                    .controlling_terminal
                    .lock_irq()
                    .clone(),
            ),
        });

        self.add_child(this.clone());
        this.signals().copy_from(self.signals());

        this
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
        let mut children = self.children.lock_irq();

        if child.clink.is_linked() {
            let mut cursor = unsafe { children.cursor_mut_from_ptr(child) };

            child.set_parent(None);
            cursor.remove();
        }
    }

    fn add_child(&self, child: Arc<Task>) {
        let mut children = self.children.lock_irq();

        child.set_parent(Some(self.this()));
        children.push_back(child);
    }

    pub fn exit_status(&self) -> &ExitStatus {
        self.exit_status.get().unwrap()
    }

    pub fn set_sleep_duration(&self, duration: usize) {
        self.sleep_duration.store(duration, Ordering::SeqCst);
    }

    pub fn load_sleep_duration(&self) -> usize {
        self.sleep_duration.load(Ordering::SeqCst)
    }

    pub fn waitpid(
        &self,
        pid: isize,
        status: &mut u32,
        flags: WaitPidFlags,
    ) -> SignalResult<usize> {
        if pid == -1 {
            // wait for any child process if no specific process is requested.
            //
            // NOTE: we collect all of the zombie list's process IDs with the children
            // list since the child could have been removed from the children list and
            // become a zombie before the parent had a chance to wait for it.
            let mut pids = self
                .zombies
                .list
                .lock_irq()
                .iter()
                .map(|e| e.pid().as_usize())
                .collect::<alloc::vec::Vec<_>>();

            pids.extend(self.children.lock_irq().iter().map(|e| e.pid().as_usize()));
            self.zombies.waitpid(&pids, status, flags)
        } else {
            self.zombies.waitpid(&[pid as _], status, flags)
        }
    }

    pub fn path(&self) -> Option<String> {
        self.executable
            .lock()
            .as_ref()
            .map(|e| e.absolute_path_str())
    }

    pub fn exec(
        &self,
        executable: DirCacheItem,

        argv: Option<ExecArgs>,
        envv: Option<ExecArgs>,
    ) -> Result<(), MapToError<Size4KiB>> {
        if self.cwd.read().is_none() {
            *self.cwd.write() = Some(Cwd::new())
        }

        self.file_table.close_on_exec();

        self.file_table.log();

        *self.executable.lock() = Some(executable.clone());

        let vm = self.vm();
        vm.clear();

        // Clear the signals that are pending for this task on exec.
        self.signals().clear();

        self.arch_task_mut().exec(vm, executable, argv, envv)
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
        if state == TaskState::Zombie {
            self.file_table.0.read().iter().for_each(|file| {
                if let Some(a) = file {
                    a.inode().close(*a.flags.read());
                }
            });
        }

        // if state != TaskState::Runnable {
        //     log::warn!(
        //         "Task::update_state() updated the task state to {state:?}! (pid={:?}, tid={:?})",
        //         self.pid,
        //         self.tid
        //     );

        //     crate::unwind::unwind_stack_trace();
        // }

        self.state.store(state as _, Ordering::SeqCst);
    }

    pub fn state(&self) -> TaskState {
        self.state.load(Ordering::SeqCst).into()
    }

    /// Returns the PID ID that was allocated for this task.
    pub fn pid(&self) -> TaskId {
        self.pid
    }

    pub fn parent_pid(&self) -> TaskId {
        self.get_parent().unwrap().pid()
    }

    pub fn tid(&self) -> TaskId {
        self.tid
    }

    pub fn cwd_dirent(&self) -> DirCacheItem {
        self.cwd.read().as_ref().unwrap().inode.clone()
    }

    pub fn get_cwd(&self) -> String {
        self.cwd.read().as_ref().unwrap().inode.absolute_path_str()
    }

    pub fn set_cwd(&self, cwd: DirCacheItem) {
        let filesystem = cwd.inode().weak_filesystem().unwrap().upgrade().unwrap();

        self.cwd.write().as_mut().unwrap().inode = cwd;
        self.cwd.write().as_mut().unwrap().filesystem = filesystem;
    }

    pub fn get_parent(&self) -> Option<Arc<Task>> {
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
        self.detach();
        self.arch_task_mut().dealloc();

        if let Some(parent) = self.get_parent() {
            parent.remove_child(self);
            parent.zombies.add_zombie(self.this());

            if self.is_process_leader() {
                parent.signal(aero_syscall::signal::SIGCHLD);
            }
        }
    }

    pub fn systrace(&self) -> bool {
        self.systrace.load(Ordering::SeqCst)
    }

    pub fn enable_systrace(&self) {
        self.systrace.store(true, Ordering::SeqCst);
    }

    pub fn detach(&self) {
        let mut controlling_terminal = self.controlling_terminal.lock_irq();

        if let Some(term) = controlling_terminal.as_ref() {
            term.detach(self.sref.upgrade().unwrap());
            *controlling_terminal = None;
        }
    }

    pub fn attach(&self, terminal: Arc<dyn TerminalDevice>) {
        if self.is_session_leader() {
            terminal.attach(self.sref.upgrade().unwrap());
            *self.controlling_terminal.lock_irq() = Some(terminal);
        } else {
            // FIXME: If its not the session leader then we needs to be a part of the the same
            // session as the terminal's foreground group.
            *self.controlling_terminal.lock_irq() = Some(terminal);
        }
    }

    /// Returns the controlling terminal of the task.
    pub fn controlling_terminal(&self) -> Option<Arc<dyn TerminalDevice>> {
        self.controlling_terminal.lock_irq().clone()
    }

    /// Returns whether the task is the session leader (`pid` == `sid`).
    pub fn is_session_leader(&self) -> bool {
        self.session_id() == self.pid().as_usize()
    }

    /// Returns whether the task is the group leader (`pid` == `gid`).
    pub fn is_group_leader(&self) -> bool {
        self.group_id() == self.pid().as_usize()
    }

    /// Returns the group identifier of the task (`GID`).
    pub fn group_id(&self) -> usize {
        self.gid.load(Ordering::SeqCst)
    }

    /// Returns the session identifier of the task (`SID`).
    pub fn session_id(&self) -> usize {
        self.sid.load(Ordering::SeqCst)
    }

    /// Sets the session identifier of the task (`SID`) to `session_id`.
    pub(super) fn set_session_id(&self, session_id: usize) {
        self.sid.store(session_id, Ordering::SeqCst);
    }

    /// Sets the group identifier of the task (`GID`) to `group_id`.
    pub(super) fn set_group_id(&self, group_id: usize) {
        self.gid.store(group_id, Ordering::SeqCst);
    }
}

// SAFETY: It's alright to access [`Task`] through references from other
// threads because we're either accessing constant properties or properties
// that are fully synchronized.
unsafe impl Sync for Task {}

intrusive_collections::intrusive_adapter!(pub SchedTaskAdapter = Arc<Task> : Task { link: LinkedListLink });
intrusive_collections::intrusive_adapter!(pub TaskAdapter = Arc<Task> : Task { clink: LinkedListLink });
