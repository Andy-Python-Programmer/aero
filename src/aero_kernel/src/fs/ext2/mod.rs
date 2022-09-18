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

use core::mem::MaybeUninit;

use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};

use crate::utils::CeilDiv;

use super::block::BlockDevice;

use super::cache;
use super::cache::{CachedINode, DirCacheItem, INodeCacheItem};

use super::inode::{DirEntry, INodeInterface, Metadata};
use super::FileSystem;

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct SuperBlock {
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub r_blocks_count: u32,
    pub free_blocks_count: u32,
    pub free_inodes_count: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub log_frag_size: u32,
    pub blocks_per_group: u32,
    pub frags_per_group: u32,
    pub inodes_per_group: u32,
    pub mtime: u32,
    pub wtime: u32,
    pub mnt_count: u16,
    pub max_mnt_count: u16,
    pub magic: u16,
    pub state: u16,
    pub errors: u16,
    pub minor_rev_level: u16,
    pub lastcheck: u32,
    pub checkinterval: u32,
    pub creator_os: u32,
    pub rev_level: u32,
    pub def_resuid: u16,
    pub def_gid: u16,

    // Extended Superblock fields
    //
    // XXX: If version number >= 1, we have to use the ext2 extended superblock as well :)
    pub first_ino: u32,
    pub inode_size: u16,
    pub block_group_nr: u16,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub uuid: [u64; 2usize],
    pub volume_name: [u8; 16usize],
    pub last_mounted: [u64; 8usize],
    pub compression_info: u32,
    pub prealloc_blocks: u8,
    pub prealloc_dir_blocks: u8,
    pub reserved_gdt_blocks: u16,
    pub journal_uuid: [u8; 16usize],
    pub journal_inum: u32,
    pub journal_dev: u32,
    pub last_orphan: u32,
    pub hash_seed: [u32; 4usize],
    pub def_hash_version: u8,
    pub jnl_backup_type: u8,
    pub group_desc_size: u16,
    pub default_mount_opts: u32,
    pub first_meta_bg: u32,
    pub mkfs_time: u32,
    pub jnl_blocks: [u32; 17usize],
}

impl SuperBlock {
    const MAGIC: u16 = 0xef53;

    /// Returns the size of a block in bytes.
    pub fn block_size(&self) -> usize {
        1024usize << self.log_block_size
    }

    /// Returns the length of the BGDT.
    pub fn bgdt_len(&self) -> usize {
        self.blocks_count.ceil_div(self.blocks_per_group) as usize
    }

    /// Returns the sector where the BGDT starts.
    pub fn bgdt_sector(&self) -> usize {
        // XXX: The block group descriptors are always located in the block immediately
        // following the superblock.
        match self.block_size() {
            1024 => 4,
            x if x > 1024 => x / 512,
            _ => unreachable!(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GroupDescriptor {
    pub block_bitmap: u32,
    pub inode_bitmap: u32,
    pub inode_table: u32,
    pub free_blocks_count: u16,
    pub free_inodes_count: u16,
    pub used_dirs_count: u16,
    pub pad: u16,
    pub reserved: [u8; 12usize],
}

const_assert_eq!(core::mem::size_of::<GroupDescriptor>(), 32);

pub enum FileType {
    Fifo,
    CharDev,
    Directory,
    BlockDev,
    File,
    Symlink,
    Socket,
    Unknown,
}

impl From<FileType> for super::inode::FileType {
    fn from(ty: FileType) -> Self {
        match ty {
            FileType::Symlink => Self::Symlink,
            FileType::Directory => Self::Directory,
            FileType::BlockDev | FileType::CharDev => Self::Device,

            _ => Self::File,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct DiskINode {
    type_and_perm: u16,
    pub user_id: u16,
    pub size_lower: u32,
    pub last_access: u32,
    pub creation_time: u32,
    pub last_modification: u32,
    pub deletion_time: u32,
    pub group_id: u16,
    pub hl_count: u16,
    pub sector_count: u32,
    pub flags: u32,
    pub os_specific: u32,
    pub data_ptr: [u32; 15],
    pub gen_number: u32,
    pub ext_attr_block: u32,
    pub size_or_acl: u32,
    pub fragment_address: u32,
    pub os_specific2: [u8; 12],
}

impl DiskINode {
    pub fn file_type(&self) -> FileType {
        let ty = self.type_and_perm >> 12;

        match ty {
            0x1 => FileType::Fifo,
            0x2 => FileType::CharDev,
            0x4 => FileType::Directory,
            0x6 => FileType::BlockDev,
            0x8 => FileType::File,
            0xa => FileType::Symlink,
            0xc => FileType::Socket,
            _ => FileType::Unknown,
        }
    }
}

const_assert_eq!(core::mem::size_of::<DiskINode>(), 128);

struct INode {
    id: usize,
    fs: Weak<Ext2>,
    inode: Box<DiskINode>,
}

impl INode {
    pub fn new(ext2: Weak<Ext2>, id: usize) -> Option<INodeCacheItem> {
        debug_assert!(id != 0);

        let icache = cache::icache();

        // Check if the inode is in the cache.
        if let Some(inode) = icache.get(INodeCacheItem::make_key(ext2.clone(), id)) {
            Some(inode)
        } else {
            let fs = ext2.upgrade()?;

            let inode_block_group = (id - 1) / fs.superblock.inodes_per_group as usize;
            let inode_table_idx = (id - 1) % fs.superblock.inodes_per_group as usize;

            let group_descriptor = &fs.bgdt[inode_block_group];
            let inode_size = core::mem::size_of::<DiskINode>(); // TODO: the inode size can be different

            let table_offset = group_descriptor.inode_table as usize * fs.superblock.block_size();
            let inode_offset = table_offset + (inode_size * inode_table_idx);

            let mut inode = Box::<DiskINode>::new_uninit();
            fs.block
                .device()
                .read(inode_offset / 512, inode.as_bytes_mut());

            // SAFETY: We have initialized the inode above.
            let inode = unsafe { inode.assume_init() };

            Some(icache.make_item_cached(CachedINode::new(Arc::new(Self {
                inode,
                id,
                fs: ext2,
            }))))
        }
    }
}

impl INodeInterface for INode {
    fn weak_filesystem(&self) -> Option<Weak<dyn FileSystem>> {
        Some(self.fs.clone())
    }

    fn metadata(&self) -> super::Result<Metadata> {
        Ok(Metadata {
            id: self.id,
            file_type: self.inode.file_type().into(),
            size: self.inode.size_lower as _,
            children_len: 0,
        })
    }
}

pub struct Ext2 {
    superblock: Box<SuperBlock>,
    bgdt: Box<[GroupDescriptor]>,
    block: Arc<BlockDevice>,

    sref: Weak<Self>,
}

impl Ext2 {
    const ROOT_INODE_ID: usize = 2;

    pub fn new(block: Arc<BlockDevice>) -> Option<Arc<Self>> {
        let mut superblock = Box::<SuperBlock>::new_uninit();
        block.device().read(2, superblock.as_bytes_mut())?;

        // SAFETY: We have initialized the superblock above.
        let superblock = unsafe { superblock.assume_init() };

        if superblock.magic != SuperBlock::MAGIC {
            return None;
        }

        let bgdt_len = superblock.bgdt_len();
        let mut bgdt = Box::<[GroupDescriptor]>::new_uninit_slice(bgdt_len);

        block.device().read(
            superblock.bgdt_sector(),
            MaybeUninit::slice_as_bytes_mut(&mut bgdt),
        )?;

        // SAFETY: We have initialized the BGD (Block Group Descriptor Table) above.
        let bgdt = unsafe { bgdt.assume_init() };

        Some(Arc::new_cyclic(|sref| Self {
            bgdt,
            superblock,
            block,

            sref: sref.clone(),
        }))
    }

    pub fn find_inode(&self, id: usize) -> Option<INodeCacheItem> {
        INode::new(self.sref.clone(), id)
    }
}

impl FileSystem for Ext2 {
    fn root_dir(&self) -> DirCacheItem {
        let inode = self
            .find_inode(Ext2::ROOT_INODE_ID)
            .expect("ext2: invalid filesystem (root inode not found)");

        DirEntry::new_root(inode, String::from("/"))
    }
}
