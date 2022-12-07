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

use core::sync::atomic::{AtomicU32, Ordering};

use aero_syscall::Termios;
use aero_syscall::WinSize;
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::sync::Weak;
use alloc::vec::Vec;
use spin::{Once, RwLock};

use uapi::pty::*;

use crate::fs::cache;
use crate::fs::cache::*;
use crate::fs::devfs;
use crate::fs::devfs::DEV_FILESYSTEM;
use crate::fs::inode::FileType;
use crate::fs::inode::{DirEntry, INodeInterface};
use crate::fs::FileSystem;
use crate::fs::Path;
use crate::fs::MOUNT_MANAGER;
use crate::fs::{self, FileSystemError};

use crate::mem::paging::VirtAddr;
use crate::utils::sync::BlockQueue;
use crate::utils::sync::Mutex;

lazy_static::lazy_static! {
    static ref PTMX: Arc<Ptmx> = Arc::new(Ptmx::new());
}

static PTS_FS: Once<Arc<PtsFs>> = Once::new();
static PTY_ID: AtomicU32 = AtomicU32::new(0);

struct Master {
    id: u32,
    wq: BlockQueue,
    slave_buffer: Mutex<Vec<u8>>,
    buffer: Mutex<Vec<u8>>,
}

impl Master {
    pub fn new() -> Self {
        Self {
            id: PTY_ID.fetch_add(1, Ordering::SeqCst),
            wq: BlockQueue::new(),
            slave_buffer: Mutex::new(Vec::new()),
            buffer: Mutex::new(Vec::new()),
        }
    }
}

impl INodeInterface for Master {
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        let mut pty_buffer = self.wq.block_on(&self.buffer, |e| !e.is_empty())?;
        let size = core::cmp::min(pty_buffer.len(), buffer.len());

        buffer[..size].copy_from_slice(&pty_buffer.drain(..size).collect::<Vec<_>>());
        Ok(size)
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        let mut pty_buffer = self.slave_buffer.lock_irq();
        pty_buffer.extend_from_slice(buffer);

        self.wq.notify_complete();
        Ok(buffer.len())
    }

    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            TIOCGPTN => {
                let id = VirtAddr::new(arg as u64)
                    .read_mut::<u32>()
                    .ok_or(FileSystemError::NotSupported)?;

                *id = self.id;
            }

            _ => {
                log::warn!("ptmx: unknown ioctl (command={command:#x})")
            }
        }

        Ok(0)
    }
}

struct SlaveInner {
    window_size: WinSize,
    termios: Termios,
}

struct Slave {
    master: Arc<Master>,
    inner: Mutex<SlaveInner>,
}

impl Slave {
    pub fn new(master: Arc<Master>) -> Self {
        Self {
            master,
            inner: Mutex::new(SlaveInner {
                window_size: WinSize::default(),
                termios: Termios {
                    c_iflag: 0,
                    c_oflag: aero_syscall::TermiosOFlag::empty(),
                    c_cflag: aero_syscall::TermiosCFlag::empty(),
                    c_lflag: aero_syscall::TermiosLFlag::ECHO | aero_syscall::TermiosLFlag::ICANON,
                    c_line: 0,
                    c_cc: [0; 32],
                    c_ispeed: 0,
                    c_ospeed: 0,
                },
            }),
        }
    }
}

impl INodeInterface for Slave {
    fn metadata(&self) -> fs::Result<fs::inode::Metadata> {
        Ok(fs::inode::Metadata {
            id: 0,
            file_type: FileType::Device,
            children_len: 0,
            size: 0,
        })
    }

    fn stat(&self) -> fs::Result<aero_syscall::Stat> {
        Ok(aero_syscall::Stat::default())
    }

    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        let mut inner = self.inner.lock_irq();

        match command {
            aero_syscall::TIOCSWINSZ => {
                let winsize = VirtAddr::new(arg as u64)
                    .read_mut::<WinSize>()
                    .ok_or(FileSystemError::NotSupported)?;

                inner.window_size = *winsize;
                Ok(0)
            }

            aero_syscall::TIOCGWINSZ => {
                let winsize = VirtAddr::new(arg as u64)
                    .read_mut::<WinSize>()
                    .ok_or(FileSystemError::NotSupported)?;

                *winsize = inner.window_size;
                Ok(0)
            }

            aero_syscall::TCGETS => {
                let termios = VirtAddr::new(arg as u64)
                    .read_mut::<Termios>()
                    .ok_or(FileSystemError::NotSupported)?;

                *termios = inner.termios;
                Ok(0)
            }

            aero_syscall::TCSETSF => {
                let termios = VirtAddr::new(arg as u64)
                    .read_mut::<Termios>()
                    .ok_or(FileSystemError::NotSupported)?;

                inner.termios = *termios;
                Ok(0)
            }

            _ => Err(FileSystemError::NotSupported),
        }
    }

    fn poll(&self, _table: Option<&mut fs::inode::PollTable>) -> fs::Result<fs::inode::PollFlags> {
        panic!()
    }

    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        let mut pty_buffer = self
            .master
            .wq
            .block_on(&self.master.slave_buffer, |e| !e.is_empty())?;

        let size = core::cmp::min(pty_buffer.len(), buffer.len());

        buffer[..size].copy_from_slice(&pty_buffer.drain(..size).collect::<Vec<_>>());
        Ok(size)
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        let mut pty_buffer = self.master.buffer.lock_irq();
        pty_buffer.extend_from_slice(buffer);

        self.master.wq.notify_complete();
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
    fn open(
        &self,
        _flags: aero_syscall::OpenFlags,
        _handle: Arc<fs::file_table::FileHandle>,
    ) -> fs::Result<Option<DirCacheItem>> {
        let master = Arc::new(Master::new());
        let slave = Arc::new(Slave::new(master.clone()));
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

        Ok(DirEntry::new(
            dir.clone(),
            inode.clone(),
            String::from(name),
        ))
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

    let fs = PTS_FS.call_once(|| PtsFs::new());

    let root = DEV_FILESYSTEM.root_dir().inode();
    root.mkdir("pts").unwrap();

    let pts_dir = fs::lookup_path(Path::new("/dev/pts")).unwrap();
    MOUNT_MANAGER.mount(pts_dir, fs.clone()).unwrap();
}

crate::module_init!(pty_init, ModuleType::Other);
