use alloc::{boxed::Box, sync::Arc};

use super::{block::BlockDevice, FileSystem};

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

pub struct Ext2 {}

impl Ext2 {
    pub fn new(device: Arc<BlockDevice>) -> Option<Arc<Self>> {
        let mut superblock = Box::<SuperBlock>::new_uninit();
        device.device().read(2, superblock.as_bytes_mut())?;

        // SAFETY: We have initialized the superblock above.
        let superblock = unsafe { superblock.assume_init() };

        if superblock.magic != 0xef53 {
            return None;
        }

        Some(Arc::new(Self {}))
    }
}

impl FileSystem for Ext2 {
    fn root_dir(&self) -> super::cache::DirCacheItem {
        todo!()
    }
}
