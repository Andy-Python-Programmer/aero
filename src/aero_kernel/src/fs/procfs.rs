use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};

use crate::utils::downcast;
use spin::{Once, RwLock};

use crate::fs::inode::FileType;

use super::cache::{CacheWeak, CachedINode, DirCacheItem, INodeCacheItem, INodeCacheWeakItem};
use super::{cache, FileSystemError};

use super::inode::{DirEntry, INodeInterface, Metadata};
use super::{FileSystem, Path, Result, MOUNT_MANAGER};

fn push_string_if_some(map: &mut serde_json::Value, key: &str, value: Option<String>) {
    if let Some(value) = value {
        map[key] = serde_json::Value::String(value);
    }
}

fn get_cpuinfo_cached() -> &'static str {
    static CACHED: Once<String> = Once::new();

    CACHED.call_once(|| {
        use alloc::vec;
        use serde_json::*;

        let mut data = json!({ "processors": [] });

        data.get_mut("processors")
            .and_then(|processors| processors.as_array_mut())
            .map(|processors| {
                let mut cpu_info = vec![];

                crate::tls::for_cpu_info_cached(|info| {
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
            });

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
    None,
}

impl Default for FileContents {
    fn default() -> Self {
        Self::None
    }
}

struct LockedProcINode(RwLock<ProcINode>);

impl LockedProcINode {
    #[inline]
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

        downcast::<dyn INodeInterface, LockedProcINode>(&inode_cached.inner())
            .expect("Failed to downcast cached inode on creation")
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
    fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        let this = self.0.read();

        match &this.contents {
            FileContents::CpuInfo => {
                let data = get_cpuinfo_cached();

                let count = core::cmp::min(buffer.len(), data.len() - offset);
                buffer[..count].copy_from_slice(&data.as_bytes()[offset..]);

                Ok(count)
            }

            _ => Err(FileSystemError::NotSupported),
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

    fn metadata(&self) -> Result<Metadata> {
        let this = self.0.read();

        Ok(Metadata {
            id: this.id,
            file_type: this.file_type,
            size: 0,
            children_len: this.children.len(),
        })
    }

    fn dirent(&self, parent: DirCacheItem, index: usize) -> Result<Option<DirCacheItem>> {
        let this = self.0.read();

        if this.file_type != FileType::Directory {
            return Err(FileSystemError::NotDirectory);
        }

        Ok(match index {
            0x00 => Some(DirEntry::new(
                parent,
                // UNWRAP: The inner node value should not be dropped.
                this.node.upgrade().unwrap().into(),
                String::from("."),
            )),

            0x01 => {
                Some(DirEntry::new(
                    parent,
                    // UNWRAP: The inner node value should not be dropped.
                    this.node.upgrade().unwrap().into(),
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
}

struct ProcFs {
    root_inode: INodeCacheItem,
    root_dir: DirCacheItem,
    next_id: AtomicUsize,
}

impl ProcFs {
    pub fn new() -> Result<Arc<Self>> {
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

        let down = downcast::<dyn INodeInterface, LockedProcINode>(root_cached.inner())
            .expect("cannot downcast inode to ram inode");

        down.init(
            &ramfs.root_inode.downgrade(),
            &&root_cached.downgrade(),
            &Arc::downgrade(&ramfs),
            FileType::Directory,
        );

        down.make_inode("cpuinfo", FileType::File, FileContents::CpuInfo)?;
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

pub fn init() -> Result<()> {
    let fs = ProcFs::new()?;
    let fs = PROC_FS.call_once(|| fs);

    let inode = super::lookup_path(Path::new("/proc"))?;
    MOUNT_MANAGER.mount(inode, fs.clone())?;

    Ok(())
}
