/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};

use spin::RwLock;

use crate::utils::downcast;

use super::cache;
use super::cache::{CachedINode, DirCacheItem, INodeCacheItem, INodeCacheWeakItem};
use super::inode::{DirEntry, FileType, INodeInterface};
use super::{FileSystem, FilesystemError, Result};

#[derive(Default)]
pub struct RamINode {
    id: usize,
    parent: INodeCacheWeakItem,
    node: INodeCacheWeakItem,
    children: BTreeMap<String, INodeCacheItem>,
    filesystem: Weak<RamFs>,
    file_type: FileType,
}

pub struct LockedRamINode(RwLock<RamINode>);

impl LockedRamINode {
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

    fn make_inode(&self, name: &str, file_type: FileType) -> Result<INodeCacheItem> {
        let icache = cache::icache();
        let mut this = self.0.write();

        if this.children.contains_key(name) || ["", ".", ".."].contains(&name) {
            return Err(FilesystemError::EntryExists);
        }

        let filesystem = this
            .filesystem
            .upgrade()
            .expect("Failed to upgrade to strong filesystem");

        let inode = filesystem.allocate_inode(file_type);
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
    fn mkdir(&self, name: &str) -> Result<INodeCacheItem> {
        self.make_inode(name, FileType::Directory)
    }

    fn lookup(&self, dir: DirCacheItem, name: &str) -> Result<DirCacheItem> {
        let this = self.0.read();
        let child = this
            .children
            .get(name)
            .ok_or(FilesystemError::EntryNotFound)?;

        Ok(DirEntry::new(
            dir.clone(),
            child.clone(),
            String::from(name),
        ))
    }

    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        Some(self.0.read().filesystem.clone())
    }
}

/// Implementation of in-memory filesystem. This is used for temporary filesystems (e.g. dev, temp) and
/// since Aero currently does not have support for actual disk filesystems (e.g. ex2 and FAT32), ram-fs is
/// used as the root filesystem.
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

    fn allocate_inode(&self, file_type: FileType) -> Arc<LockedRamINode> {
        Arc::new(LockedRamINode::new(RamINode {
            parent: Weak::default(),
            node: Weak::default(),
            filesystem: Weak::default(),
            children: BTreeMap::new(),
            id: self.next_id.fetch_add(1, Ordering::SeqCst),
            file_type,
        }))
    }
}

impl FileSystem for RamFs {
    fn root_dir(&self) -> DirCacheItem {
        self.root_dir.clone()
    }
}
