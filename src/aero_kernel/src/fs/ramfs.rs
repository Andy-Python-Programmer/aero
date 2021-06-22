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

//! Implementation of in-memory filesystem. This is used for temporary filesystems (e.g. dev, temp) and
//! since Aero currently does not have support for actual disk filesystems (e.g. ex2 and FAT32), ram-fs is
//! used as the root filesystem.

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};

use spin::RwLock;

use crate::utils::downcast;

use super::cache;
use super::cache::{CachedINode, DirCacheItem, INodeCacheItem, INodeCacheWeakItem};
use super::devfs::DevINode;
use super::inode::{DirEntry, FileType, INodeInterface};
use super::inode::{FileContents, Metadata};
use super::{FileSystem, FileSystemError, Result};

#[derive(Default)]
pub struct RamINode {
    id: usize,
    parent: INodeCacheWeakItem,
    node: INodeCacheWeakItem,
    children: BTreeMap<String, INodeCacheItem>,
    filesystem: Weak<RamFs>,
    file_type: FileType,
    contents: FileContents,
}

pub struct LockedRamINode(RwLock<RamINode>);

impl LockedRamINode {
    #[inline]
    fn new(node: RamINode) -> Self {
        Self(RwLock::new(node))
    }

    fn init(
        &self,
        parent: &INodeCacheWeakItem,
        node: &INodeCacheWeakItem,
        filesystem: &Weak<RamFs>,
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
    ) -> Result<INodeCacheItem> {
        let icache = cache::icache();
        let mut this = self.0.write();

        if this.children.contains_key(name) || ["", ".", ".."].contains(&name) {
            return Err(FileSystemError::EntryExists);
        }

        let filesystem = this
            .filesystem
            .upgrade()
            .expect("Failed to upgrade to strong filesystem");

        let inode = filesystem.allocate_inode(file_type, contents);
        let inode_cached = icache.make_item_no_cache(CachedINode::new(inode));

        downcast::<dyn INodeInterface, LockedRamINode>(&inode_cached.inner())
            .expect("Failed to downcast cached inode on creation")
            .init(
                &this.node,
                &Arc::downgrade(&inode_cached),
                &this.filesystem,
                file_type,
            );

        this.children
            .insert(String::from(name), inode_cached.clone());

        Ok(inode_cached)
    }
}

impl INodeInterface for LockedRamINode {
    #[inline]
    fn mkdir(&self, name: &str) -> Result<INodeCacheItem> {
        self.make_inode(name, FileType::Directory, FileContents::None)
    }

    #[inline]
    fn make_dev_inode(&self, name: &str, marker: usize) -> Result<INodeCacheItem> {
        self.make_inode(
            name,
            FileType::Device,
            FileContents::Device(DevINode::new(marker)?),
        )
    }

    fn metadata(&self) -> Metadata {
        let this = self.0.read();

        Metadata {
            id: this.id,
            file_type: this.file_type,
            size: match &this.contents {
                FileContents::Content(bytes) => bytes.lock().len(), // Temporary value dropped and lock is unlocked!
                _ => 0x00,
            },
            children_len: this.children.len(),
        }
    }

    fn lookup(&self, dir: DirCacheItem, name: &str) -> Result<DirCacheItem> {
        let this = self.0.read();
        let child = this
            .children
            .get(name)
            .ok_or(FileSystemError::EntryNotFound)?;

        Ok(DirEntry::new(
            dir.clone(),
            child.clone(),
            String::from(name),
        ))
    }

    #[inline]
    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        Some(self.0.read().filesystem.clone())
    }
}

/// Implementation of in-memory filesystem. (See the module-level documentation for more
/// information).
pub struct RamFs {
    root_inode: INodeCacheItem,
    root_dir: DirCacheItem,
    next_id: AtomicUsize,
}

impl RamFs {
    pub fn new() -> Arc<Self> {
        let icache = cache::icache();

        let root_node = Arc::new(LockedRamINode::new(RamINode::default()));
        let root_cached = icache.make_item_no_cache(CachedINode::new(root_node));

        let root_dir = DirEntry::new_root(root_cached.clone(), String::from("/"));

        let ramfs = Arc::new(Self {
            root_inode: root_cached.clone(),
            root_dir: root_dir.clone(),
            next_id: AtomicUsize::new(0x00),
        });

        let copy: Arc<dyn FileSystem> = ramfs.clone();

        root_dir.filesystem.call_once(|| Arc::downgrade(&copy));

        downcast::<dyn INodeInterface, LockedRamINode>(root_cached.inner())
            .expect("Cannot downcast inode to ram inode")
            .init(
                &Arc::downgrade(&ramfs.root_inode),
                &Arc::downgrade(&root_cached),
                &Arc::downgrade(&ramfs),
                FileType::Directory,
            );

        ramfs
    }

    fn allocate_inode(&self, file_type: FileType, contents: FileContents) -> Arc<LockedRamINode> {
        Arc::new(LockedRamINode::new(RamINode {
            parent: Weak::default(),
            node: Weak::default(),
            filesystem: Weak::default(),
            children: BTreeMap::new(),
            id: self.next_id.fetch_add(1, Ordering::SeqCst),
            contents,
            file_type,
        }))
    }
}

impl FileSystem for RamFs {
    #[inline]
    fn root_dir(&self) -> DirCacheItem {
        self.root_dir.clone()
    }
}
