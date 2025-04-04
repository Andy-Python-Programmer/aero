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

use core::sync::atomic::{AtomicUsize, Ordering};

use aero_syscall::OpenFlags;
use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};

use spin::{Once, RwLock};

use crate::fs;
use crate::fs::inode::FileType;

use crate::arch::tls;
use crate::userland::scheduler;

use super::cache::*;
use super::{cache, FileSystem, Path, MOUNT_MANAGER};

use super::inode::{DirEntry, INodeInterface, Metadata};
use super::FileSystemError;

// TODO: put this mf in prelude
use alloc::vec;

fn push_string_if_some(map: &mut serde_json::Value, key: &str, value: Option<String>) {
    if let Some(value) = value {
        map[key] = serde_json::Value::String(value);
    }
}

fn get_cmdline_cached() -> &'static str {
    static CACHED: Once<String> = Once::new();

    CACHED.call_once(|| {
        use serde_json::*;

        json!({
            "cmdline": crate::cmdline::get_raw_cmdline(),
        })
        .to_string()
    })
}

fn get_cpuinfo_cached() -> &'static str {
    static CACHED: Once<String> = Once::new();

    CACHED.call_once(|| {
        use serde_json::*;

        let mut data = json!({ "processors": [] });

        if let Some(processors) = data
            .get_mut("processors")
            .and_then(|processors| processors.as_array_mut())
        {
            let mut cpu_info = vec![];

            #[cfg(target_arch = "x86_64")]
            tls::for_cpu_info_cached(|info| {
                let mut processor = json!({});

                processor["id"] = Value::Number(Number::from(info.cpuid));
                processor["fpu"] = Value::Bool(info.fpu);

                push_string_if_some(&mut processor, "brand", info.brand.clone());
                push_string_if_some(&mut processor, "vendor", info.vendor.clone());

                processor["features"] = Value::Array(
                    info.features
                        .iter()
                        .map(|feature| Value::String(feature.to_string()))
                        .collect(),
                );

                cpu_info.push(processor);
            });

            *processors = cpu_info;
        }

        data.to_string()
    })
}

#[derive(Default)]
struct ProcINode {
    id: usize,
    parent: INodeCacheWeakItem,
    node: INodeCacheWeakItem,
    children: BTreeMap<String, INodeCacheItem>,
    filesystem: Weak<ProcFs>,
    file_type: FileType,
    contents: FileContents,
}

enum FileContents {
    CpuInfo,
    CmdLine,
    SelfMaps,

    None,
}

impl Default for FileContents {
    fn default() -> Self {
        Self::None
    }
}

struct LockedProcINode(RwLock<ProcINode>);

impl LockedProcINode {
    fn new(node: ProcINode) -> Self {
        Self(RwLock::new(node))
    }

    fn init(
        &self,
        parent: &INodeCacheWeakItem,
        node: &INodeCacheWeakItem,
        filesystem: &Weak<ProcFs>,
        file_type: FileType,
    ) {
        let mut this = self.0.write();

        this.parent = parent.clone();
        this.node = node.clone();
        this.filesystem = filesystem.clone();
        this.file_type = file_type;
    }

    fn make_inode(
        &self,
        name: &str,
        file_type: FileType,
        contents: FileContents,
    ) -> fs::Result<INodeCacheItem> {
        let icache = cache::icache();
        let mut this = self.0.write();

        if this.children.contains_key(name) || ["", ".", ".."].contains(&name) {
            return Err(FileSystemError::EntryExists);
        }

        let filesystem = this.filesystem.upgrade().unwrap();

        let inode = filesystem.allocate_inode(file_type, contents);
        let inode_cached = icache.make_item_no_cache(CachedINode::new(inode));

        inode_cached
            .inner()
            .downcast_arc::<LockedProcINode>()
            .unwrap()
            .init(
                &this.node,
                &inode_cached.downgrade(),
                &this.filesystem,
                file_type,
            );

        this.children
            .insert(String::from(name), inode_cached.clone());

        Ok(inode_cached)
    }
}

impl INodeInterface for LockedProcINode {
    fn read_at(&self, _flags: OpenFlags, offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        let this = self.0.read();

        let data = match &this.contents {
            FileContents::CpuInfo => Ok(get_cpuinfo_cached().to_owned()),
            FileContents::CmdLine => Ok(get_cmdline_cached().to_owned()),

            FileContents::SelfMaps => {
                let current_thread = scheduler::current_thread();
                let mut result = serde_json::json!({ "maps": [] });
                let maps = result.get_mut("maps").unwrap().as_array_mut().unwrap();

                current_thread.vm().for_each_mapping(|map| {
                    maps.push(serde_json::json!({
                        "start": map.start_addr.as_u64(),
                        "end": map.end_addr.as_u64(),
                        // "flags": map.flags.bits(),
                        // do we need to tell if is shared?
                        "protection": map.protection().bits(),
                    }));
                });

                Ok(result.to_string())
            }

            _ => Err(FileSystemError::NotSupported),
        }?;

        let count = core::cmp::min(buffer.len(), data.len() - offset);
        buffer[..count].copy_from_slice(&data.as_bytes()[offset..offset + count]);

        Ok(count)
    }

    fn lookup(&self, dir: DirCacheItem, name: &str) -> fs::Result<DirCacheItem> {
        let this = self.0.read();
        let child = this
            .children
            .get(name)
            .ok_or(FileSystemError::EntryNotFound)?;

        Ok(DirEntry::new(dir, child.clone(), String::from(name)))
    }

    fn metadata(&self) -> fs::Result<Metadata> {
        let this = self.0.read();

        Ok(Metadata {
            id: this.id,
            file_type: this.file_type,
            size: 0,
            children_len: this.children.len(),
        })
    }

    fn dirent(&self, parent: DirCacheItem, index: usize) -> fs::Result<Option<DirCacheItem>> {
        let this = self.0.read();

        if this.file_type != FileType::Directory {
            return Err(FileSystemError::NotDirectory);
        }

        Ok(match index {
            0x00 => Some(DirEntry::new(
                parent,
                // UNWRAP: The inner node value should not be dropped.
                this.node.upgrade().unwrap(),
                String::from("."),
            )),

            0x01 => {
                Some(DirEntry::new(
                    parent,
                    // UNWRAP: The inner node value should not be dropped.
                    this.node.upgrade().unwrap(),
                    String::from(".."),
                ))
            }

            // Subtract two because of the "." and ".." entries.
            _ => this
                .children
                .iter()
                .nth(index - 2)
                .map(|(name, inode)| DirEntry::new(parent, inode.clone(), name.clone())),
        })
    }

    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        Some(self.0.read().filesystem.clone())
    }
}

struct ProcFs {
    root_inode: INodeCacheItem,
    root_dir: DirCacheItem,
    next_id: AtomicUsize,
}

impl ProcFs {
    pub fn new() -> fs::Result<Arc<Self>> {
        let icache = cache::icache();

        let root_node = Arc::new(LockedProcINode::new(ProcINode::default()));
        let root_cached = icache.make_item_no_cache(CachedINode::new(root_node));

        let root_dir = DirEntry::new_root(root_cached.clone(), String::from("/"));

        let ramfs = Arc::new(Self {
            root_inode: root_cached.clone(),
            root_dir: root_dir.clone(),
            next_id: AtomicUsize::new(0x00),
        });

        let copy: Arc<dyn FileSystem> = ramfs.clone();

        root_dir.filesystem.call_once(|| Arc::downgrade(&copy));

        let inode = root_cached
            .inner()
            .downcast_arc::<LockedProcINode>()
            .unwrap();

        inode.init(
            &ramfs.root_inode.downgrade(),
            &root_cached.downgrade(),
            &Arc::downgrade(&ramfs),
            FileType::Directory,
        );

        inode.make_inode("cpuinfo", FileType::File, FileContents::CpuInfo)?;
        inode.make_inode("cmdline", FileType::File, FileContents::CmdLine)?;

        let proc_self = inode.make_inode("self", FileType::Directory, FileContents::None)?;
        let proc_self = proc_self.downcast_arc::<LockedProcINode>().unwrap();

        proc_self.make_inode("maps", FileType::File, FileContents::SelfMaps)?;

        Ok(ramfs)
    }

    fn allocate_inode(&self, file_type: FileType, contents: FileContents) -> Arc<LockedProcINode> {
        Arc::new(LockedProcINode::new(ProcINode {
            parent: CacheWeak::new(),
            node: CacheWeak::new(),
            filesystem: Weak::default(),
            children: BTreeMap::new(),
            id: self.next_id.fetch_add(1, Ordering::SeqCst),
            contents,
            file_type,
        }))
    }
}

impl FileSystem for ProcFs {
    #[inline]
    fn root_dir(&self) -> DirCacheItem {
        self.root_dir.clone()
    }
}

static PROC_FS: Once<Arc<ProcFs>> = Once::new();

pub fn init() -> fs::Result<()> {
    let fs = ProcFs::new()?;
    let fs = PROC_FS.call_once(|| fs);

    let inode = super::lookup_path(Path::new("/proc"))?;
    MOUNT_MANAGER.mount(inode, fs.clone())?;

    Ok(())
}
