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

mod disk;
mod group_desc;

use core::mem::MaybeUninit;

use aero_syscall::socket::{MessageFlags, MessageHeader};
use aero_syscall::{MMapFlags, SyscallError};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use spin::RwLock;

use crate::fs::block::{BlockDeviceInterface, DirtyRef};
use crate::fs::cache::CachedINode;
use crate::fs::ext2::disk::{FileType, Revision, SuperBlock};
use crate::mem::paging::*;

use crate::socket::unix::UnixSocket;
use crate::socket::SocketAddrRef;

use self::group_desc::GroupDescriptors;

use super::block::{self, BlockDevice, CachedAccess};

use super::cache::{DirCacheItem, INodeCacheItem};
use super::{cache, FileSystemError};

use super::inode::{DirEntry, INodeInterface, Metadata, PollFlags, PollTable};
use super::FileSystem;

pub struct INode {
    id: usize,
    fs: Weak<Ext2>,
    inode: RwLock<Box<disk::INode>>,
    // Forwards all of the inode operations to the proxy inode. Note that the
    // proxy inode is not saved on the disk. (e.g. This is useful for binding
    // a socket inode to a file).
    proxy: Option<Arc<dyn INodeInterface>>,

    // TODO: Do not store this in the inode, but rather in a different
    // cache using the API provided by fs::cache (consider LRU only?).
    sref: Weak<INode>,
}

impl INode {
    pub fn new(
        ext2: Weak<Ext2>,
        id: usize,
        proxy: Option<Arc<dyn INodeInterface>>,
    ) -> Option<INodeCacheItem> {
        debug_assert!(id != 0);

        let icache = cache::icache();

        // Check if the inode is in the cache.
        if let Some(inode) = icache.get(INodeCacheItem::make_key(ext2.clone(), id)) {
            Some(inode)
        } else {
            let fs = ext2.upgrade()?;
            let inode = fs.bgdt.find_inode(id)?;

            Some(
                icache.make_item_cached(CachedINode::new(Arc::new_cyclic(|sref| Self {
                    inode: RwLock::new(inode),
                    id,
                    fs: ext2,
                    proxy,

                    sref: sref.clone(),
                }))),
            )
        }
    }

    /// Reads the data at `offset` as `T`.
    ///
    /// ## Safety
    /// * The data being read should be of a valid value for `T`.
    pub unsafe fn read_mut<T: Sized>(&self, offset: usize) -> block::DirtyRef<T> {
        assert!(core::mem::size_of::<T>() <= Size4KiB::SIZE as usize);

        let filesystem = self.fs.upgrade().unwrap();
        let block_size = filesystem.superblock.block_size();

        let block = offset / block_size;
        let loc = offset % block_size;

        let block_index = self.get_block(block).unwrap() as usize;

        block::DirtyRef::new(filesystem.block.sref(), (block_index * block_size) + loc)
    }

    pub fn read(&self, offset: usize, buffer: &mut [MaybeUninit<u8>]) -> super::Result<usize> {
        let inode = self.inode.read();
        let filesystem = self.fs.upgrade().unwrap();
        let block_size = filesystem.superblock.block_size();

        let mut progress = 0;
        let count = core::cmp::min(inode.size() - offset, buffer.len());

        while progress < count {
            let block = (offset + progress) / block_size;
            let loc = (offset + progress) % block_size;

            let mut chunk = count - progress;

            if chunk > block_size - loc {
                chunk = block_size - loc;
            }

            let block_index = self.get_block(block).unwrap() as usize;

            filesystem
                .block
                .read(
                    (block_index * block_size) + loc,
                    &mut buffer[progress..progress + chunk],
                )
                .expect("inode: read failed");

            progress += chunk;
        }

        Ok(count)
    }

    pub fn write(&self, offset: usize, buffer: &[u8]) -> super::Result<usize> {
        let filesystem = self.fs.upgrade().unwrap();
        let block_size = filesystem.superblock.block_size();

        let mut progress = 0;
        let count = buffer.len();

        while progress < count {
            let block = (offset + progress) / block_size;
            let loc = (offset + progress) % block_size;

            let mut chunk = count - progress;

            if chunk > block_size - loc {
                chunk = block_size - loc;
            }

            let mut block_index = self.get_block(block).unwrap() as usize;

            if block_index == 0 {
                block_index = self.append_block().unwrap();
            }

            filesystem
                .block
                .write(
                    (block_index * block_size) + loc,
                    &buffer[progress..progress + chunk],
                )
                .expect("inode: write failed");

            progress += chunk;
        }

        Ok(count)
    }

    pub fn append_block(&self) -> Option<usize> {
        let fs = self.fs.upgrade().expect("ext2: filesystem was dropped");
        let block_size = fs.superblock.block_size();
        let entries_per_block = fs.superblock.entries_per_block();

        let new_block = fs.bgdt.alloc_block_ptr()?;

        let mut next_block_num = self.inode.read().size().div_ceil(block_size);

        if next_block_num < 12 {
            let mut inode = self.inode.write();

            assert_eq!(inode.data_ptr[next_block_num], 0);

            inode.data_ptr[next_block_num] = new_block as u32;

            let size = inode.size() + block_size;
            inode.set_size(size);

            return Some(new_block);
        }

        // indirect block
        next_block_num -= 12;

        if next_block_num >= entries_per_block {
            unimplemented!("append_block: doubly and triply indirect")
        } else {
            // singly indirect block
            let mut block_ptrs = self.inode.read().data_ptr[12] as usize;

            if block_ptrs == 0 {
                block_ptrs = fs.bgdt.alloc_block_ptr()?;
                self.inode.write().data_ptr[12] = block_ptrs as u32;
            }

            let block_ptrs = block_ptrs * block_size;
            let offset = block_ptrs + (next_block_num * core::mem::size_of::<u32>());

            fs.block.write(offset, &new_block.to_le_bytes());

            let mut inode = self.inode.write();
            let inode_size = inode.size() + block_size;
            inode.set_size(inode_size);
        }

        Some(new_block)
    }

    pub fn get_block(&self, mut block: usize) -> Option<u32> {
        // There are pointers to the first 12 blocks which contain the file's
        // data in the inode. There is a pointer to an indirect block (which
        // contains pointers to the next set of blocks), a pointer to a doubly
        // indirect block and a pointer to a triply indirect block.
        if block < 12 {
            // direct block
            return Some(self.inode.read().data_ptr[block]);
        }

        // indirect block
        block -= 12;

        let fs = self.fs.upgrade()?;
        let superblock = &fs.superblock;

        let entries_per_block = superblock.entries_per_block();
        let block_size = superblock.block_size();

        if block >= entries_per_block {
            // doubly indirect block
            block -= entries_per_block;

            let index = block / entries_per_block;
            let mut indirect_block = MaybeUninit::<u32>::uninit();

            if index >= entries_per_block {
                // treply indirect block
                todo!()
            } else {
                let block_ptrs = self.inode.read().data_ptr[13] as usize * block_size;
                let offset = block_ptrs + (index * core::mem::size_of::<u32>());

                fs.block
                    .read(offset, indirect_block.as_bytes_mut())
                    .unwrap();
            }

            // SAFETY: We have initialized the variable above.
            let indirect_block = unsafe { indirect_block.assume_init() } as usize * block_size;
            let offset = indirect_block + (block % entries_per_block) * core::mem::size_of::<u32>();

            let mut res = MaybeUninit::<u32>::uninit();
            fs.block.read(offset, res.as_bytes_mut());

            // SAFETY: We have initialized the variable above.
            Some(unsafe { res.assume_init() })
        } else {
            // singly indirect block
            let block_ptrs = self.inode.read().data_ptr[12] as usize * block_size;
            let offset = block_ptrs + (block * core::mem::size_of::<u32>());

            let mut res = MaybeUninit::<u32>::uninit();
            fs.block.read(offset, res.as_bytes_mut());

            // SAFETY: We have initialized the variable above.
            Some(unsafe { res.assume_init() })
        }
    }

    pub fn make_disk_dirent(&self, inode: Arc<INode>, file_type: u8, name: &str) {
        // TODO: scan for unused directory entries and check if this can be
        //       inserted into the existing block.
        let block = self.append_block().unwrap();
        let fs = self.fs.upgrade().expect("ext2: filesystem was dropped");
        let block_size = fs.superblock.block_size();

        let mut entry = DirtyRef::<disk::DirEntry>::new(fs.block.sref(), block * block_size);
        entry.entry_size = block_size as _;
        entry.inode = inode.id as _;
        entry.file_type = file_type;
        entry.set_name(name);
    }

    pub fn make_inode(
        &self,
        name: &str,
        typ: FileType,
        proxy: Option<Arc<dyn INodeInterface>>,
    ) -> super::Result<INodeCacheItem> {
        if !self.metadata()?.is_directory() {
            return Err(FileSystemError::NotDirectory);
        }

        if DirEntryIter::new(self.sref()).any(|(e, _)| e == name) {
            return Err(FileSystemError::EntryExists);
        }

        assert!(self.inode.read().hl_count != 0, "ext2: dangling inode");

        let fs = self.fs.upgrade().expect("ext2: filesystem was dropped");

        let inode = fs.bgdt.alloc_inode().expect("ext2: out of inodes");
        let inode = fs.find_inode(inode, proxy).expect("ext2: inode not found");

        let ext2_inode = inode.downcast_arc::<INode>().expect("ext2: invalid inode");

        {
            let mut inode = ext2_inode.inode.write();
            **inode = disk::INode::default();

            inode.set_file_type(typ);
            inode.set_permissions(0o755);

            inode.hl_count += 1;
        }

        // FIXME: Fix the filetype!
        self.make_disk_dirent(ext2_inode, 2, name);
        Ok(inode)
    }

    pub fn make_dirent(
        &self,
        parent: DirCacheItem,
        name: &str,
        entry: &disk::DirEntry,
    ) -> Option<DirCacheItem> {
        let inode = self.fs.upgrade()?.find_inode(entry.inode as usize, None)?;
        Some(DirEntry::new(parent, inode, name.to_string()))
    }

    pub fn sref(&self) -> Arc<INode> {
        self.sref.upgrade().unwrap()
    }
}

impl INodeInterface for INode {
    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        Some(self.fs.clone())
    }

    fn metadata(&self) -> super::Result<Metadata> {
        let inode = self.inode.read();

        Ok(Metadata {
            id: self.id,
            file_type: inode.file_type().into(),
            size: inode.size(),
            children_len: 0,
        })
    }

    fn stat(&self) -> super::Result<aero_syscall::Stat> {
        use super::inode::FileType;
        use aero_syscall::{Mode, Stat};

        let inode = self.inode.read();

        let filesystem = self.fs.upgrade().unwrap();
        let filetype = self.metadata()?.file_type();

        let mut mode = Mode::empty();

        match filetype {
            FileType::File => mode.insert(Mode::S_IFREG),
            FileType::Directory => mode.insert(Mode::S_IFDIR),
            FileType::Device => mode.insert(Mode::S_IFCHR),
            FileType::Socket => mode.insert(Mode::S_IFSOCK),
            FileType::Symlink => mode.insert(Mode::S_IFLNK),
        }

        // FIXME: read permission bits from the inode.
        mode.insert(Mode::S_IRWXU | Mode::S_IRWXG | Mode::S_IRWXO);

        Ok(Stat {
            st_ino: self.id as _,
            st_blksize: filesystem.superblock.block_size() as _,
            st_size: inode.size() as _,
            st_mode: mode,

            ..Default::default()
        })
    }

    fn dirent(&self, parent: DirCacheItem, index: usize) -> super::Result<Option<DirCacheItem>> {
        if let Some((name, entry)) = DirEntryIter::new(self.sref()).nth(index) {
            Ok(self.make_dirent(parent, &name, &entry))
        } else {
            Ok(None)
        }
    }

    fn lookup(&self, parent: DirCacheItem, name: &str) -> super::Result<DirCacheItem> {
        let (name, entry) = DirEntryIter::new(self.sref())
            .find(|(ename, _)| ename == name)
            .ok_or(FileSystemError::EntryNotFound)?;

        Ok(self.make_dirent(parent, &name, &entry).unwrap())
    }

    fn read_at(&self, offset: usize, usr_buffer: &mut [u8]) -> super::Result<usize> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.read_at(offset, usr_buffer);
        }

        if !self.metadata()?.is_file() {
            return Err(FileSystemError::NotSupported);
        }

        let buffer = unsafe {
            core::slice::from_raw_parts_mut(usr_buffer.as_mut_ptr().cast(), usr_buffer.len())
        };

        self.read(offset, buffer)
    }

    fn write_at(&self, offset: usize, usr_buffer: &[u8]) -> super::Result<usize> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.write_at(offset, usr_buffer);
        }

        if !self.metadata()?.is_file() && !self.metadata()?.is_symlink() {
            return Err(FileSystemError::NotSupported);
        }

        self.write(offset, usr_buffer)
    }

    fn rename(&self, old: DirCacheItem, dest: &str) -> super::Result<()> {
        assert!(self.metadata()?.is_directory());

        if DirEntryIter::new(self.sref()).any(|(name, _)| name == dest) {
            return Err(FileSystemError::EntryExists);
        }

        if let Some(_parent) = old.parent() {
            // FIXME: Remove the directory entry from the parent
            self.make_disk_dirent(old.inode().downcast_arc().unwrap(), 2, dest);
            return Ok(());
        }

        Err(FileSystemError::NotSupported)
    }

    fn link(&self, name: &str, src: DirCacheItem) -> super::Result<()> {
        if !self.metadata()?.is_directory() {
            return Err(FileSystemError::NotSupported);
        }

        if src.inode().metadata()?.is_directory() {
            return Err(FileSystemError::NotSupported);
        }

        let inode = self.make_inode(name, FileType::Symlink, None)?;
        inode.write_at(0, src.name().as_bytes())?;

        Ok(())
    }

    fn truncate(&self, size: usize) -> super::Result<()> {
        let inode = self.inode.read();

        if inode.size() > size {
            // grow inode
            log::warn!("ext2::truncate(at=grow) is a stub!");
        } else if inode.size() < size {
            log::warn!("ext2::truncate(at=shrink) is a stub!");
            // shrink inode
        }

        Ok(())
    }

    fn touch(&self, parent: DirCacheItem, name: &str) -> super::Result<DirCacheItem> {
        if !self.metadata()?.is_directory() {
            return Err(FileSystemError::NotDirectory);
        }

        let inode = self.make_inode(name, FileType::File, None)?;
        Ok(DirEntry::new(parent, inode, name.to_string()))
    }

    fn mkdir(&self, name: &str) -> super::Result<INodeCacheItem> {
        if !self.metadata()?.is_directory() {
            return Err(FileSystemError::NotDirectory);
        }

        self.make_inode(name, FileType::Directory, None)
    }

    fn make_local_socket_inode(
        &self,
        name: &str,
        inode: Arc<dyn INodeInterface>,
    ) -> super::Result<INodeCacheItem> {
        self.make_inode(name, FileType::Socket, Some(inode))
    }

    fn resolve_link(&self) -> super::Result<String> {
        if !self.metadata()?.is_symlink() {
            return Err(FileSystemError::NotSupported);
        }

        let inode = self.inode.read();

        let path_len = inode.size();
        let data_bytes: &[u8] = bytemuck::cast_slice(&inode.data_ptr);

        if path_len <= data_bytes.len() {
            let path_bytes = &data_bytes[..path_len];
            let path = core::str::from_utf8(path_bytes).or(Err(FileSystemError::InvalidPath))?;

            Ok(path.into())
        } else {
            let mut buffer = Box::<[u8]>::new_uninit_slice(path_len);
            self.read(0, MaybeUninit::slice_as_bytes_mut(&mut buffer))?;

            let path_bytes = unsafe { buffer.assume_init() };
            let path = core::str::from_utf8(&path_bytes).or(Err(FileSystemError::InvalidPath))?;

            Ok(path.into())
        }
    }

    fn mmap(&self, offset: usize, size: usize, flags: MMapFlags) -> super::Result<PhysFrame> {
        assert!(self.proxy.is_none());

        // TODO: support shared file mappings.
        // assert!(!flags.contains(MMapFlags::MAP_SHARED));

        let private_cp: PhysFrame = FRAME_ALLOCATOR.allocate_frame().unwrap();
        private_cp.as_slice_mut().fill(0);

        let buffer = &mut private_cp.as_slice_mut()[..size];
        self.read_at(offset, buffer)?;

        Ok(private_cp)
    }

    fn listen(&self, backlog: usize) -> Result<(), SyscallError> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.listen(backlog);
        }

        Err(SyscallError::EACCES)
    }

    // XXX: We do not require to handle `bind` here since if this function
    // is being is called on an EXT2 inode then, it has already been bound.

    fn connect(&self, address: SocketAddrRef, length: usize) -> super::Result<()> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.connect(address, length);
        }

        Err(FileSystemError::NotSupported)
    }

    fn accept(&self, address: Option<(VirtAddr, &mut u32)>) -> super::Result<Arc<UnixSocket>> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.accept(address);
        }

        Err(FileSystemError::NotSupported)
    }

    fn send(&self, message_hdr: &mut MessageHeader, flags: MessageFlags) -> super::Result<usize> {
        if let Some(proxy) = self.proxy.as_ref() {
            proxy.send(message_hdr, flags)
        } else {
            Err(FileSystemError::NotSupported)
        }
    }

    fn recv(&self, message_hdr: &mut MessageHeader, flags: MessageFlags) -> super::Result<usize> {
        if let Some(proxy) = self.proxy.as_ref() {
            proxy.recv(message_hdr, flags)
        } else {
            Err(FileSystemError::NotSupported)
        }
    }

    fn poll(&self, table: Option<&mut PollTable>) -> super::Result<PollFlags> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.poll(table);
        }

        Err(FileSystemError::NotSupported)
    }

    fn as_unix_socket(&self) -> super::Result<Arc<dyn INodeInterface>> {
        self.proxy
            .as_ref()
            .ok_or(FileSystemError::NotSocket)
            .cloned()
    }
}

pub struct DirEntryIter {
    inode: Arc<INode>,
    offset: usize,
}

impl DirEntryIter {
    pub fn new(inode: Arc<INode>) -> Self {
        Self { inode, offset: 0 }
    }
}

impl Iterator for DirEntryIter {
    type Item = (String, block::DirtyRef<disk::DirEntry>);

    fn next(&mut self) -> Option<Self::Item> {
        let file_size = self.inode.inode.read().size();

        if self.offset + core::mem::size_of::<disk::DirEntry>() > file_size {
            return None;
        }

        let entry = unsafe { self.inode.read_mut::<disk::DirEntry>(self.offset) };
        if entry.inode == 0 {
            return None;
        }

        let mut name = Box::<[u8]>::new_uninit_slice(entry.name_size as usize);
        self.inode
            .read(
                self.offset + core::mem::size_of::<disk::DirEntry>(),
                MaybeUninit::slice_as_bytes_mut(&mut name),
            )
            .ok()?;

        // SAFETY: We have initialized the name above.
        let name = unsafe { name.assume_init() };
        let name = unsafe { core::str::from_utf8_unchecked(&name) };

        self.offset += entry.entry_size as usize;
        Some((name.to_string(), entry))
    }
}

pub struct Ext2 {
    superblock: Box<SuperBlock>,
    bgdt: GroupDescriptors,
    block: Arc<BlockDevice>,

    sref: Weak<Self>,
}

impl Ext2 {
    const ROOT_INODE_ID: usize = 2;

    pub fn new(block: Arc<BlockDevice>) -> Option<Arc<Self>> {
        let mut superblock = Box::<SuperBlock>::new_uninit();
        block.read_block(2, superblock.as_bytes_mut())?;

        // SAFETY: We have initialized the superblock above.
        let superblock = unsafe { superblock.assume_init() };

        if superblock.magic != SuperBlock::MAGIC {
            return None;
        }

        log::trace!(
            "ext2: initialized (block_size={}, entries_per_block={})",
            superblock.block_size(),
            superblock.entries_per_block(),
        );

        assert_eq!(superblock.revision(), Revision::Revision1);
        assert_eq!(
            superblock.inode_size as usize,
            core::mem::size_of::<disk::INode>()
        );

        Some(Arc::new_cyclic(|sref| Self {
            bgdt: GroupDescriptors::new(sref.clone(), block.clone(), &superblock)
                .expect("ext2: failed to read group descriptors"),
            superblock,
            block,

            sref: sref.clone(),
        }))
    }

    pub fn find_inode(
        &self,
        id: usize,
        proxy: Option<Arc<dyn INodeInterface>>,
    ) -> Option<INodeCacheItem> {
        INode::new(self.sref.clone(), id, proxy)
    }
}

impl FileSystem for Ext2 {
    fn root_dir(&self) -> DirCacheItem {
        let inode = self
            .find_inode(Ext2::ROOT_INODE_ID, None)
            .expect("ext2: invalid filesystem (root inode not found)");

        DirEntry::new_root(inode, String::from("/"))
    }
}
