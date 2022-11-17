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

use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::sync::Arc;
use spin::{Once, RwLock};
use uapi::pty::TIOCGPTN;

use crate::fs::cache;
use crate::fs::cache::*;
use crate::fs::devfs;
use crate::fs::inode::{DirEntry, INodeInterface};
use crate::fs::{self, FileSystemError};

use crate::mem::paging::VirtAddr;

lazy_static::lazy_static! {
    static ref PTMX: Arc<Ptmx> = Arc::new(Ptmx::new());
}

static PTS_FS: Once<Arc<PtsFs>> = Once::new();
static PTY_ID: AtomicU32 = AtomicU32::new(0);

struct Master {
    id: u32,
}

impl Master {
    pub fn new() -> Self {
        Self {
            id: PTY_ID.fetch_add(1, Ordering::SeqCst),
        }
    }
}

impl INodeInterface for Master {
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        log::warn!("Master::read: is a stub!");
        Ok(buffer.len())
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        log::warn!("PTY::Master::read: is a stub!");
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

struct Slave {
    master: Arc<Master>,
}

impl Slave {
    pub fn new(master: Arc<Master>) -> Self {
        Self { master }
    }
}

impl INodeInterface for Slave {
    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> fs::Result<usize> {
        panic!()
    }

    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> fs::Result<usize> {
        panic!()
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
    slaves: RwLock<BTreeMap<u32, INodeCacheItem>>,
}

impl INodeInterface for PtsINode {
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

            _ => self
                .slaves
                .read()
                .iter()
                .nth(index)
                .map(|(id, inode)| DirEntry::new(parent, inode.clone(), id.to_string())),
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

        pts_root.inode.call_once(|| root_inode.clone());

        Arc::new(Self { root_dir })
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
    PTS_FS.call_once(|| PtsFs::new());
}

crate::module_init!(pty_init);
