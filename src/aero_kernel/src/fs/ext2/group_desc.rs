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

use bit_field::BitField;
use spin::RwLock;

use crate::fs::block::{BlockDevice, CachedAccess};

use super::{disk, Ext2};

pub struct GroupDescriptors {
    descriptors: RwLock<Box<[disk::GroupDescriptor]>>,
    ext2: Weak<Ext2>,
}

impl GroupDescriptors {
    /// Reads the block group descriptors from the disk.
    ///
    /// ## Arguments
    ///
    /// * `ext2` - Weak pointer to the EXT2 filesystem. This function does not call `upgrade` on
    ///            the pointer so, it is valid to pass a semi-constructed `Arc<T>` pointer (e.g
    ///            through `Arc::new_cyclic`) though invoking other functions on the group descriptors
    ///            require the pointer to be fully constructed).
    ///
    /// * `superblock` - Reference to the EXT2 superblock.
    ///
    /// * `device` - Block device to read the group descriptors from.
    pub fn new(
        ext2: Weak<Ext2>,
        device: Arc<BlockDevice>,
        superblock: &disk::SuperBlock,
    ) -> Option<Self> {
        let bgdt_len = superblock.bgdt_len();
        let mut bgdt = Box::<[disk::GroupDescriptor]>::new_uninit_slice(bgdt_len);

        device.read(
            superblock.bgdt_block(),
            MaybeUninit::slice_as_bytes_mut(&mut bgdt),
        )?;

        // SAFETY: We have initialized the BGD (Block Group Descriptor Table) above.
        let bgdt = unsafe { bgdt.assume_init() };
        Some(Self {
            descriptors: RwLock::new(bgdt),
            ext2,
        })
    }

    // XXX: The free inodes are managed by bitmaps. An EXT2 filesystem contains
    // several block groups where each group contains a bitmap for blocks and
    // a bitmap for inodes and their free counts. The group descriptors are located
    // after the super block.

    /// Returns the index of the block group which has free block(s)
    /// avaliable.
    pub fn find_free_block(&self) -> Option<usize> {
        let (index, _) = self
            .descriptors
            .read()
            .iter()
            .enumerate()
            .find(|(_, e)| e.free_blocks_count >= 1)?;

        Some(index)
    }

    /// Returns the index of the block group which has free inode(s)
    /// avaliable.
    pub fn find_free_inode(&self) -> Option<usize> {
        let (index, _) = self
            .descriptors
            .read()
            .iter()
            .enumerate()
            .find(|(_, e)| e.free_inodes_count >= 1)?;

        Some(index)
    }

    pub fn find_inode(&self, id: usize) -> Option<Box<disk::INode>> {
        let fs = self.ext2.upgrade()?;
        let this = self.descriptors.read();
        let superblock = &fs.superblock;

        // There is one inode table per block group and can be located by
        // the `inode_table` offset in the group descriptor. Also there are
        // `inodes_per_group` inodes per table.
        let ino_per_group = superblock.inodes_per_group as usize;

        let ino_block_group = (id - 1) / ino_per_group;
        let ino_table_index = (id - 1) % ino_per_group;

        let group_descriptor = this[ino_block_group];
        let table_offset = group_descriptor.inode_table as usize * superblock.block_size();

        let mut inode = Box::<disk::INode>::new_uninit();

        fs.block.read(
            table_offset + (ino_table_index * core::mem::size_of::<disk::INode>()),
            inode.as_bytes_mut(),
        )?;

        // SAFETY: We have initialized the inode above.
        let inode = unsafe { inode.assume_init() };
        Some(inode)
    }

    /// Allocates a block pointer using the first fit allocation strategy.
    pub fn alloc_block_ptr(&self) -> Option<usize> {
        let fs = self.ext2.upgrade()?;
        let blocks_per_group = fs.superblock.blocks_per_group as usize;

        if let Some(block_group_idx) = self.find_free_block() {
            let mut descriptors = self.descriptors.write();
            let block_group = &mut descriptors[block_group_idx];

            let mut bitmap = Bitmap::new(fs.clone(), block_group.block_bitmap as usize)?;
            let block_id = block_group_idx * blocks_per_group + bitmap.alloc()?;

            block_group.free_blocks_count -= 1;
            drop(descriptors);

            // TODO: decrement the number of free blocks in the superblock.
            return Some(block_id);
        }

        None
    }

    /// Allocates a new inode using the first fit allocation strategy.
    pub fn alloc_inode(&self) -> Option<usize> {
        let fs = self.ext2.upgrade()?;
        let ino_per_group = fs.superblock.inodes_per_group as usize;

        if let Some(block_group_idx) = self.find_free_inode() {
            let mut descriptors = self.descriptors.write();
            let block_group = &mut descriptors[block_group_idx];

            let mut bitmap = Bitmap::new(fs.clone(), block_group.inode_bitmap as usize)?;
            // Since inode numbers start from 1 rather than 0, the first bit in the first block
            // group's inode bitmap represent inode number 1. Thus, we add 1 to the allocated
            // inode number.
            let inode_id = block_group_idx * ino_per_group + bitmap.alloc()? + 1;

            block_group.free_inodes_count -= 1;
            drop(descriptors); // release the lock

            return Some(inode_id);
        }

        None
    }
}

struct Bitmap {
    bitmap: Box<[u8]>,
    fs: Weak<Ext2>,
    offset: usize,
}

impl Bitmap {
    /// Reads the bitmap at `block` from the disk. The bitmap is required have
    /// a size of `block_size` bytes.
    ///
    /// **Note**: Any changes to the bitmap will be written back to the disk when the
    /// bitmap has been dropped.
    fn new(fs: Arc<Ext2>, block: usize) -> Option<Self> {
        let block_size = fs.superblock.block_size();
        let offset = block * block_size;

        let mut bitmap = Box::<[u8]>::new_uninit_slice(block_size);

        fs.block.read(offset, &mut bitmap)?;

        // SAFETY: We have initialized the bitmap above.
        let bitmap = unsafe { bitmap.assume_init() };

        Some(Self {
            bitmap,
            offset,
            fs: Arc::downgrade(&fs),
        })
    }

    /// Allocates a free bit in the bitmap and returns its index.
    pub fn alloc(&mut self) -> Option<usize> {
        for (i, e) in self.bitmap.iter_mut().enumerate() {
            if *e != u8::MAX {
                for bit in 0..8 {
                    if e.get_bit(bit) == false {
                        e.set_bit(bit, true);

                        return Some(i * 8 + bit);
                    }
                }
            }
        }

        None
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        let fs = self
            .fs
            .upgrade()
            .expect("ext2: filesystem has been dropped");

        fs.block.write(self.offset, &self.bitmap);
    }
}
