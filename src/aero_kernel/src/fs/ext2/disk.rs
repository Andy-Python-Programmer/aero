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

use bit_field::BitField;

use crate::fs::inode;
use crate::utils::CeilDiv;

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
    pub const MAGIC: u16 = 0xef53;

    /// Returns the number of entries per block.
    pub fn entries_per_block(&self) -> usize {
        self.block_size() / core::mem::size_of::<u32>()
    }

    /// Returns the size of a block in bytes.
    pub fn block_size(&self) -> usize {
        1024usize << self.log_block_size
    }

    /// Returns the length of the BGDT.
    pub fn bgdt_len(&self) -> usize {
        self.blocks_count.ceil_div(self.blocks_per_group) as usize
    }

    pub fn bgdt_block(&self) -> usize {
        // XXX: The block group descriptors are always located in the block immediately
        // following the superblock.
        let block_size = self.block_size();

        if block_size >= 2048 {
            block_size
        } else {
            block_size * 2
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

#[derive(Debug, Copy, Clone)]
#[repr(C, packed)]
pub struct DirEntry {
    pub inode: u32,
    pub entry_size: u16,
    pub name_size: u8,
    pub file_type: u8,
}

impl DirEntry {
    pub fn set_name(&mut self, name: &str) {
        assert!(name.len() < u8::MAX as usize);

        self.name_size = name.len() as u8;

        // SAFETY: Above we have verified that the name will fit in the entry.
        let name_ptr = unsafe { (self as *mut _ as *mut u8).add(core::mem::size_of::<Self>()) };
        let name_bytes = unsafe { core::slice::from_raw_parts_mut(name_ptr, name.len()) };

        name_bytes.copy_from_slice(name.as_bytes());
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, self.entry_size as usize)
        }
    }
}

#[repr(u8)]
#[derive(PartialEq, Copy, Clone)]
pub enum FileType {
    Unknown = 0,
    Fifo = 1,
    CharDev = 2,
    Directory = 4,
    BlockDev = 6,
    File = 8,
    Symlink = 10,
    Socket = 12,
}

impl FileType {
    pub fn bits(&self) -> u16 {
        let val = *self as u8;
        (val as u16) << 12
    }
}

impl From<FileType> for inode::FileType {
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
pub struct INode {
    type_and_perm: u16,
    pub user_id: u16,
    pub size_lower: u32,
    pub last_access: u32,
    pub creation_time: u32,
    pub last_modification: u32,
    pub deletion_time: u32,
    pub group_id: u16,
    pub hl_count: u16,
    pub block_count: u32,
    pub flags: u32,
    pub os_specific: u32,
    pub data_ptr: [u32; 15],
    pub gen_number: u32,
    pub ext_attr_block: u32,
    pub size_or_acl: u32,
    pub fragment_address: u32,
    pub os_specific2: [u8; 12],
}

impl INode {
    pub fn set_file_type(&mut self, file_type: FileType) {
        // The last 4 bits are used to store the filetype.
        let mask = 0b0000_1111_1111_1111u16;
        self.type_and_perm = file_type.bits() | (self.type_and_perm & mask);
    }

    pub fn set_permissions(&mut self, permissions: u16) {
        let mut val = self.type_and_perm;
        val.set_bits(..13, permissions);
        self.type_and_perm = val;
    }

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

const_assert_eq!(core::mem::size_of::<INode>(), 128);
