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

mod disk;
mod group_desc;

use core::mem::MaybeUninit;

use aero_syscall::socket::MessageHeader;
use aero_syscall::{MMapFlags, SyscallError};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use spin::RwLock;

use crate::fs::block::BlockDeviceInterface;
use crate::fs::cache::CachedINode;
use crate::fs::ext2::disk::{FileType, SuperBlock};
use crate::mem::paging::{FrameAllocator, PhysFrame, VirtAddr, FRAME_ALLOCATOR};

use crate::socket::unix::UnixSocket;
use crate::socket::SocketAddr;

use self::group_desc::GroupDescriptors;

use super::block::{BlockDevice, CachedAccess};

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

    pub fn read(&self, offset: usize, buffer: &mut [MaybeUninit<u8>]) -> super::Result<usize> {
        let inode = self.inode.read();
        let filesystem = self.fs.upgrade().unwrap();
        let block_size = filesystem.superblock.block_size();

        let mut progress = 0;
        let count = core::cmp::min(inode.size_lower as usize - offset, buffer.len());

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

        let new_block = fs.bgdt.alloc_block_ptr()?;
        let data_ptrs = self.inode.read().data_ptr;

        // Check if the there are free direct data pointers avaliable to
        // insert the new block..
        for (i, block) in data_ptrs[..12].iter().enumerate() {
            if *block == 0 {
                drop(data_ptrs);

                let mut inode = self.inode.write();
                inode.data_ptr[i] = new_block as u32;
                inode.size_lower += block_size as u32;

                return Some(new_block);
            }
        }

        todo!("append_block: indirect blocks")
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

        if block <= entries_per_block {
            // singly indirect block
            let block_ptrs = self.inode.read().data_ptr[12] as usize * block_size;
            let offset = block_ptrs + (block * core::mem::size_of::<u32>());

            let mut res = MaybeUninit::<u32>::uninit();
            fs.block.read(offset, res.as_bytes_mut());

            // SAFETY: We have initialized the variable above.
            return Some(unsafe { res.assume_init() });
        }

        block -= entries_per_block;

        if block <= entries_per_block {
            // doubly indirect block
            let block_ptrs = self.inode.read().data_ptr[13] as usize * block_size;
            let offset = block_ptrs + ((block / entries_per_block) * core::mem::size_of::<u32>());

            let mut indirect_block = MaybeUninit::<u32>::uninit();
            fs.block.read(offset, indirect_block.as_bytes_mut());

            // SAFETY: We have initialized the variable above.
            let indirect_block = unsafe { indirect_block.assume_init() } as usize * block_size;

            let offset = indirect_block + entries_per_block * core::mem::size_of::<u32>();

            let mut res = MaybeUninit::<u32>::uninit();
            fs.block.read(offset, res.as_bytes_mut());

            // SAFETY: We have initialized the variable above.
            return Some(unsafe { res.assume_init() });
        }

        todo!("triply indirect block")
    }

    pub fn make_inode(
        &self,
        name: &str,
        typ: FileType,
        proxy: Option<Arc<dyn INodeInterface>>,
    ) -> super::Result<INodeCacheItem> {
        if !self.metadata()?.is_directory() {
            return Err(FileSystemError::NotSupported);
        }

        if DirEntryIter::new(self.sref())
            .find(|(e, _)| e == name)
            .is_some()
        {
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

        // TODO: scan for unused directory entries and check if this can be
        //       inserted into the existing block.
        let block = self.append_block().unwrap();
        let block_size = fs.superblock.block_size();

        let mut entry = Box::<disk::DirEntry>::new_uninit();
        fs.block.read(block * block_size, entry.as_bytes_mut());

        // SAFETY: We have initialized the entry above.
        let mut entry = unsafe { entry.assume_init() };

        entry.entry_size = block_size as _;
        entry.inode = ext2_inode.id as _;
        entry.file_type = 2; // FIXME: This is fucked.
        entry.set_name(name);

        fs.block.write(block * block_size, entry.as_bytes());
        Ok(inode)
    }

    pub fn make_dir_entry(
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
            size: inode.size_lower as _,
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
            st_size: inode.size_lower as _,
            st_mode: mode,

            ..Default::default()
        })
    }

    fn dirent(&self, parent: DirCacheItem, index: usize) -> super::Result<Option<DirCacheItem>> {
        if let Some((name, entry)) = DirEntryIter::new(self.sref()).nth(index) {
            Ok(self.make_dir_entry(parent, &name, &entry))
        } else {
            Ok(None)
        }
    }

    fn lookup(&self, parent: DirCacheItem, name: &str) -> super::Result<DirCacheItem> {
        let (name, entry) = DirEntryIter::new(self.sref())
            .find(|(ename, _)| ename == name)
            .ok_or(FileSystemError::EntryNotFound)?;

        Ok(self.make_dir_entry(parent, &name, &entry).unwrap())
    }

    fn read_at(&self, offset: usize, usr_buffer: &mut [u8]) -> super::Result<usize> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.read_at(offset, usr_buffer);
        }

        if !self.metadata()?.is_file() {
            return Err(FileSystemError::NotSupported);
        }

        // TODO: We really should not allocate another buffer here.
        let mut buffer = Box::<[u8]>::new_uninit_slice(usr_buffer.len());
        let count = self.read(offset, MaybeUninit::slice_as_bytes_mut(&mut buffer))?;

        // SAFETY: We have initialized the data buffer above.
        let buffer = unsafe { buffer.assume_init() };
        usr_buffer.copy_from_slice(&*buffer);

        Ok(count)
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

    fn truncate(&self, _size: usize) -> super::Result<()> {
        log::warn!("ext2::truncate is a stub!");
        Ok(())
    }

    fn touch(&self, parent: DirCacheItem, name: &str) -> super::Result<DirCacheItem> {
        if !self.metadata()?.is_directory() {
            return Err(FileSystemError::NotSupported);
        }

        let inode = self.make_inode(name, FileType::File, None)?;
        Ok(DirEntry::new(parent, inode, name.to_string()))
    }

    fn mkdir(&self, name: &str) -> super::Result<INodeCacheItem> {
        if !self.metadata()?.is_directory() {
            return Err(FileSystemError::NotSupported);
        }

        self.make_inode(name, FileType::Directory, None)
    }

    fn make_local_socket_inode(
        &self,
        name: &str,
        inode: Arc<dyn INodeInterface>,
    ) -> super::Result<INodeCacheItem> {
        Ok(self.make_inode(name, FileType::Socket, Some(inode))?)
    }

    fn resolve_link(&self) -> super::Result<String> {
        if !self.metadata()?.is_symlink() {
            return Err(FileSystemError::NotSupported);
        }

        let inode = self.inode.read();

        let path_len = inode.size_lower as usize;
        let data_bytes: &[u8] = bytemuck::cast_slice(&inode.data_ptr);

        assert!(path_len <= data_bytes.len() - 1);

        let path_bytes = &data_bytes[..path_len];
        let path = core::str::from_utf8(path_bytes).or(Err(FileSystemError::InvalidPath))?;

        return Ok(path.into());
    }

    fn mmap(&self, offset: usize, size: usize, flags: MMapFlags) -> super::Result<PhysFrame> {
        assert!(self.proxy.is_none());

        // TODO: support shared file mappings.
        assert!(!flags.contains(MMapFlags::MAP_SHARED));

        let private_cp: PhysFrame = FRAME_ALLOCATOR.allocate_frame().unwrap();

        let buffer = &mut private_cp.as_slice_mut()[..size];
        self.read_at(offset, buffer)?;

        Ok(private_cp)
    }

    fn listen(&self, backlog: usize) -> Result<(), SyscallError> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.listen(backlog);
        }

        return Err(SyscallError::EACCES);
    }

    // XXX: We do not require to handle `bind` here since if this function
    // is being is called on an EXT2 inode then, it has already been bound.

    fn connect(&self, address: SocketAddr, length: usize) -> super::Result<()> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.connect(address, length);
        }

        return Err(FileSystemError::NotSupported);
    }

    fn accept(&self, address: Option<(VirtAddr, &mut u32)>) -> super::Result<Arc<UnixSocket>> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.accept(address);
        }

        return Err(FileSystemError::NotSupported);
    }

    fn recv(&self, message_header: &mut MessageHeader, non_block: bool) -> super::Result<usize> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.recv(message_header, non_block);
        }

        return Err(FileSystemError::NotSupported);
    }

    fn poll(&self, table: Option<&mut PollTable>) -> super::Result<PollFlags> {
        if let Some(proxy) = self.proxy.as_ref() {
            return proxy.poll(table);
        }

        return Err(FileSystemError::NotSupported);
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
    type Item = (String, Box<disk::DirEntry>);

    fn next(&mut self) -> Option<Self::Item> {
        let file_size = self.inode.inode.read().size_lower as usize;

        if self.offset + core::mem::size_of::<disk::DirEntry>() > file_size {
            return None;
        }

        let mut entry = Box::<disk::DirEntry>::new_uninit();

        self.inode.read(self.offset, entry.as_bytes_mut()).ok()?;

        // SAFETY: We have initialized the entry above.
        let entry = unsafe { entry.assume_init() };

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
        let name = unsafe { core::str::from_utf8_unchecked(&*name) };

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
