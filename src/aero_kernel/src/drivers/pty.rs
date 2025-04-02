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

use core::sync::atomic::{AtomicU32, Ordering};

use aero_syscall as libc;
use aero_syscall::{Termios, WinSize};

use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::{Once, RwLock};

use uapi::pty::TIOCGPTN;

use crate::arch::user_copy::UserRef;
use crate::fs::cache::*;
use crate::fs::devfs::DEV_FILESYSTEM;
use crate::fs::inode::{DirEntry, FileType, INodeInterface, PollFlags};
use crate::fs::{self, cache, devfs, FileSystem, FileSystemError, Path, MOUNT_MANAGER};

use crate::mem::paging::VirtAddr;
use crate::userland::scheduler;
use crate::userland::scheduler::ExitStatus;
use crate::userland::task::Task;
use crate::userland::terminal::{LineControl, LineDiscipline, TerminalDevice};
use crate::utils::sync::{Mutex, WaitQueue};

lazy_static::lazy_static! {
    static ref PTMX: Arc<Ptmx> = Arc::new(Ptmx::new());
}

static PTS_FS: Once<Arc<PtsFs>> = Once::new();
static PTY_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Ioctl)]
pub enum TermiosCmd {
    /// Get window size.
    #[command(libc::TIOCGWINSZ)]
    GetWinSize(UserRef<WinSize>),

    /// Set window size.
    #[command(libc::TIOCSWINSZ)]
    SetWinSize(UserRef<WinSize>),

    /// Get the current serial port settings.
    ///
    /// Equivalent to `tcgetattr(fd, argp)`.
    #[command(libc::TCGETS)]
    TcGets(UserRef<Termios>),

    /// Allow the output buffer to drain, discard pending input, and set the current serial
    /// port settings.
    ///
    /// Equivalent to `tcsetattr(fd, TCSAFLUSH, argp)`.
    #[command(libc::TCSETSF)]
    TcSetsf(UserRef<Termios>),

    /// Allow the output buffer to drain, and set the current serial port settings.
    ///
    /// Equivalent to `tcsetattr(fd, TCSADRAIN, argp)`.
    #[command(libc::TCSETSW)]
    TcSetsw(UserRef<Termios>),

    /// Make the given terminal the controlling terminal of the calling process. The calling
    /// process must be a session leader and not have a controlling terminal already. For this
    /// case, arg should be specified as zero.
    ///
    /// If this terminal is already the controlling terminal of a different session group, then the
    /// ioctl fails with EPERM, unless the caller has the CAP_SYS_ADMIN capability and arg equals
    /// 1, in which case the terminal is stolen, and all processes that had it as controlling
    /// terminal lose it.
    // FIXME: argument usize
    #[command(libc::TIOCSCTTY)]
    SetCtrlTerm,

    /// Get the process group ID of the foreground process group on this terminal.
    ///
    /// When successful, equivalent to `*argp = tcgetpgrp(fd)`.
    // FIXME: argument usize
    #[command(libc::TIOCGPGRP)]
    GetProcGroupId,
}

struct Master {
    id: u32,
    wq: WaitQueue,
    window_size: Mutex<WinSize>,
    buffer: Mutex<Vec<u8>>,
    discipline: LineDiscipline,
}

impl Master {
    pub fn new() -> Self {
        Self {
            id: PTY_ID.fetch_add(1, Ordering::SeqCst),
            wq: WaitQueue::new(),
            window_size: Mutex::new(WinSize::default()),
            buffer: Mutex::new(Vec::new()),
            discipline: LineDiscipline::new(),
        }
    }

    #[inline]
    fn set_window_size(&self, size: WinSize) {
        *self.window_size.lock_irq() = size;
    }

    #[inline]
    fn get_window_size(&self) -> WinSize {
        *self.window_size.lock_irq()
    }
}

impl INodeInterface for Master {
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        let mut pty_buffer = self.buffer.lock_irq();

        if pty_buffer.is_empty() {
            return Err(FileSystemError::WouldBlock);
        }

        let size = core::cmp::min(pty_buffer.len(), buffer.len());
        buffer[..size].copy_from_slice(&pty_buffer.drain(..size).collect::<Vec<_>>());
        Ok(size)
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        self.discipline.write(buffer, |ctrl| match ctrl {
            LineControl::Echo(c) => self.buffer.lock_irq().push(c),
        });
        self.wq.notify_all();
        Ok(buffer.len())
    }

    fn poll(&self, table: Option<&mut fs::inode::PollTable>) -> fs::Result<fs::inode::PollFlags> {
        if let Some(e) = table {
            e.insert(&self.wq)
        }
        let mut flags = fs::inode::PollFlags::OUT;

        if !self.buffer.lock_irq().is_empty() {
            flags |= fs::inode::PollFlags::IN;
        }

        Ok(flags)
    }

    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            TIOCGPTN => {
                let id = VirtAddr::new(arg as u64).read_mut::<u32>()?;
                *id = self.id;
            }

            aero_syscall::TIOCSWINSZ => {
                let winsize = VirtAddr::new(arg as u64).read_mut::<WinSize>()?;
                *self.window_size.lock_irq() = *winsize;
            }

            _ => {
                panic!("ptmx: unknown ioctl (command={command:#x})")
            }
        }

        Ok(0)
    }
}

impl TerminalDevice for Slave {
    fn attach(&self, task: Arc<Task>) {
        assert!(task.is_session_leader());
        self.master.discipline.set_foreground(&task);
    }

    fn detach(&self, task: Arc<Task>) {
        use aero_syscall::signal::SIGINT;
        use aero_syscall::VINTR;

        if !self.master.discipline.termios.lock().is_cooked() {
            return;
        }

        if let ExitStatus::Signal(signo) = task.exit_status() {
            let mut buffer = self.master.buffer.lock_irq();
            let termios = self.master.discipline.termios.lock();

            // converts `X` into `^X` and pushes the result into the master PTY buffer.
            let mut ctrl = |c| {
                buffer.extend_from_slice(&[b'^', c + 0x40]);
            };

            if *signo == SIGINT {
                ctrl(termios.c_cc[VINTR])
            }
        }
    }
}

struct Slave {
    sref: Weak<Self>,
    master: Arc<Master>,
}

impl Slave {
    pub fn new(master: Arc<Master>) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),
            master,
        })
    }

    fn sref(&self) -> Arc<Self> {
        self.sref.upgrade().unwrap()
    }
}

impl INodeInterface for Slave {
    fn metadata(&self) -> fs::Result<fs::inode::Metadata> {
        Ok(fs::inode::Metadata::with_file_type(FileType::Device))
    }

    fn stat(&self) -> fs::Result<aero_syscall::Stat> {
        Ok(aero_syscall::Stat::default())
    }

    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match TermiosCmd::from_command_arg(command, arg) {
            TermiosCmd::GetWinSize(mut size) => *size = self.master.get_window_size(),
            TermiosCmd::SetWinSize(size) => self.master.set_window_size(*size),
            TermiosCmd::TcGets(mut termios) => *termios = self.master.discipline.termios(),
            TermiosCmd::TcSetsf(termios) => self.master.discipline.set_termios(termios.clone()),
            TermiosCmd::TcSetsw(termios) => {
                // TODO: Allow the output buffer to drain and then set the current serial port
                // settings.
                self.master.discipline.set_termios(termios.clone())
            }

            TermiosCmd::SetCtrlTerm => {
                let current_task = scheduler::get_scheduler().current_task();
                assert!(current_task.is_session_leader());

                current_task.attach(self.sref());
            }

            // FIXME: the following ioctls are not implemented.
            TermiosCmd::GetProcGroupId => return Err(FileSystemError::NotSupported),
        }

        Ok(0)
    }

    fn poll(&self, table: Option<&mut fs::inode::PollTable>) -> fs::Result<PollFlags> {
        if let Some(table) = table {
            table.insert(&self.master.wq);
            table.insert(self.master.discipline.wait_queue());
        }

        let mut flags = PollFlags::OUT;

        if !self.master.discipline.is_empty() {
            flags |= PollFlags::IN;
        }

        Ok(flags)
    }

    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        Ok(self.master.discipline.read(buffer)?)
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        if self
            .master
            .discipline
            .termios()
            .c_oflag
            .contains(aero_syscall::TermiosOFlag::ONLCR)
        {
            let mut master = self.master.buffer.lock_irq();

            for b in buffer.iter() {
                if *b == b'\n' {
                    // ONLCR: Convert NL to CR + NL
                    master.extend_from_slice(b"\r\n");
                    continue;
                }

                master.push(*b);
            }
        } else {
            let mut pty_buffer = self.master.buffer.lock_irq();
            pty_buffer.extend_from_slice(buffer);
        }

        self.master.wq.notify_all();
        Ok(buffer.len())
    }
}

struct Ptmx {
    device_id: usize,
}

impl Ptmx {
    fn new() -> Self {
        Self {
            device_id: devfs::alloc_device_marker(),
        }
    }
}

impl devfs::Device for Ptmx {
    fn device_marker(&self) -> usize {
        self.device_id
    }

    fn device_name(&self) -> String {
        String::from("ptmx")
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        PTMX.clone()
    }
}

impl INodeInterface for Ptmx {
    fn open(&self, _handle: Arc<fs::file_table::FileHandle>) -> fs::Result<Option<DirCacheItem>> {
        let master = Arc::new(Master::new());
        let slave = Slave::new(master.clone());
        let inode = DirEntry::from_inode(master, String::from("<pty>"));

        PTS_FS.get().unwrap().insert_slave(slave);
        Ok(Some(inode))
    }
}

#[derive(Default)]
struct PtsINode {
    inode: Once<INodeCacheItem>,
    fs: Once<Weak<PtsFs>>,
    slaves: RwLock<BTreeMap<u32, INodeCacheItem>>,
}

impl INodeInterface for PtsINode {
    fn metadata(&self) -> fs::Result<fs::inode::Metadata> {
        Ok(fs::inode::Metadata {
            id: 0,
            file_type: FileType::Directory,
            children_len: self.slaves.read().len(),
            size: 0,
        })
    }

    fn stat(&self) -> fs::Result<aero_syscall::Stat> {
        Ok(aero_syscall::Stat::default())
    }

    fn dirent(&self, parent: DirCacheItem, index: usize) -> fs::Result<Option<DirCacheItem>> {
        Ok(match index {
            0x00 => Some(DirEntry::new(
                parent,
                self.inode.get().unwrap().clone(),
                String::from("."),
            )),

            0x01 => Some(DirEntry::new(
                parent,
                self.inode.get().unwrap().clone(),
                String::from(".."),
            )),

            _ => {
                let a = self
                    .slaves
                    .read()
                    .iter()
                    .nth(index - 2)
                    .map(|(id, inode)| DirEntry::new(parent, inode.clone(), id.to_string()));
                log::debug!("{}", a.is_some());
                a
            }
        })
    }

    fn lookup(&self, dir: DirCacheItem, name: &str) -> fs::Result<DirCacheItem> {
        let id = name.parse::<u32>().unwrap();
        let slaves = self.slaves.read();

        let (_, inode) = slaves
            .iter()
            .find(|(&e, _)| e == id)
            .ok_or(FileSystemError::EntryNotFound)?;

        Ok(DirEntry::new(dir, inode.clone(), String::from(name)))
    }

    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        Some(self.fs.get()?.clone())
    }
}

struct PtsFs {
    root_dir: DirCacheItem,
}

impl PtsFs {
    fn new() -> Arc<Self> {
        let icache = cache::icache();
        let root_inode = icache.make_item_no_cache(CachedINode::new(Arc::new(PtsINode::default())));

        let root_dir = DirEntry::new_root(root_inode.clone(), String::from("/"));
        let pts_root = root_dir.inode().downcast_arc::<PtsINode>().unwrap();

        let this = Arc::new(Self { root_dir });

        // Initialize the PTS root inode.
        pts_root.fs.call_once(|| Arc::downgrade(&this));
        pts_root.inode.call_once(|| root_inode.clone());

        this
    }

    fn insert_slave(&self, slave: Arc<Slave>) {
        let icache = cache::icache();

        let pts_root = self.root_dir.inode().downcast_arc::<PtsINode>().unwrap();
        pts_root.slaves.write().insert(
            slave.master.id,
            icache.make_item_no_cache(CachedINode::new(slave)),
        );
    }
}

impl fs::FileSystem for PtsFs {
    fn root_dir(&self) -> DirCacheItem {
        self.root_dir.clone()
    }
}

fn pty_init() {
    devfs::install_device(PTMX.clone()).unwrap();

    let fs = PTS_FS.call_once(PtsFs::new);

    let root = DEV_FILESYSTEM.root_dir().inode();
    root.mkdir("pts").unwrap();

    let pts_dir = fs::lookup_path(Path::new("/dev/pts")).unwrap();
    MOUNT_MANAGER.mount(pts_dir, fs.clone()).unwrap();
}

crate::module_init!(pty_init, ModuleType::Other);
