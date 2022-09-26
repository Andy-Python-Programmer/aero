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

mod gpt;

use alloc::boxed::Box;
use gpt::Gpt;

use core::mem::MaybeUninit;

use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::fs::devfs::install_device;
use crate::fs::{FileSystem, Result};

use crate::fs::ext2::Ext2;
use crate::utils::sync::Mutex;

use super::cache::{Cache, Cacheable};
use super::devfs::{alloc_device_marker, Device};
use super::inode::INodeInterface;

type CachedBlockKey = (usize, usize); // (block device pointer, block)

struct CachedBlock {
    device: Weak<dyn CachedAccess>,
    block: usize,
    buffer: Box<[u8]>,
}

impl CachedBlock {
    fn make_key(device: Weak<dyn CachedAccess>, block: usize) -> CachedBlockKey {
        (device.as_ptr() as *const u8 as usize, block)
    }
}

impl Cacheable<CachedBlockKey> for CachedBlock {
    fn cache_key(&self) -> CachedBlockKey {
        Self::make_key(self.device.clone(), self.block)
    }
}

lazy_static::lazy_static! {
    static ref BLOCK_CACHE: Arc<Cache<CachedBlockKey, CachedBlock>> = Cache::new();
}

pub trait BlockDeviceInterface: Send + Sync {
    fn block_size(&self) -> usize;

    fn read_block(&self, sector: usize, dest: &mut [MaybeUninit<u8>]) -> Option<usize>;
    fn write_block(&self, sector: usize, buf: &[u8]) -> Option<usize>;
}

pub trait CachedAccess: BlockDeviceInterface {
    fn sref(&self) -> Weak<dyn CachedAccess>;

    fn read(&self, offset: usize, dest: &mut [MaybeUninit<u8>]) -> Option<usize> {
        let mut progress = 0;
        let block_size = self.block_size();

        while progress < dest.len() {
            let block = (offset + progress) / block_size;
            let loc = (offset + progress) % block_size;

            let mut chunk = dest.len() - progress;

            if chunk > (block_size - loc) {
                chunk = block_size - loc;
            }

            let key = CachedBlock::make_key(self.sref(), block);

            if let Some(cached) = BLOCK_CACHE.get(key) {
                MaybeUninit::write_slice(
                    &mut dest[progress..(progress + chunk)],
                    &cached.buffer[loc..loc + chunk],
                );
            } else {
                let mut buffer = Box::<[u8]>::new_uninit_slice(block_size);

                self.read_block(block, MaybeUninit::slice_as_bytes_mut(&mut buffer))?;
                dest[progress..(progress + chunk)].copy_from_slice(&buffer[loc..loc + chunk]);

                BLOCK_CACHE.make_item_cached(CachedBlock {
                    device: self.sref(),
                    block,
                    buffer: unsafe { buffer.assume_init() },
                });
            }

            progress += chunk;
        }

        Some(progress)
    }
}

static BLOCK_DEVS: Mutex<BTreeMap<usize, Arc<BlockDevice>>> = Mutex::new(BTreeMap::new());

/// Installs the provided block `device` into the filesyetm.
pub fn install_block_device(dev: Arc<BlockDevice>) -> Result<()> {
    let mut devs = BLOCK_DEVS.lock();
    install_device(dev.clone())?;

    log::debug!("block: installed block device {}", dev.name());
    devs.insert(dev.id, dev);

    Ok(())
}

pub struct BlockDevice {
    id: usize,
    name: String,
    dev: Arc<dyn BlockDeviceInterface>,
    sref: Weak<BlockDevice>,
}

impl BlockDevice {
    pub fn new(name: String, imp: Arc<dyn BlockDeviceInterface>) -> Arc<BlockDevice> {
        Arc::new_cyclic(|sref| BlockDevice {
            id: alloc_device_marker(),
            name,
            dev: imp,
            sref: sref.clone(),
        })
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }
}

impl BlockDeviceInterface for BlockDevice {
    fn block_size(&self) -> usize {
        self.dev.block_size()
    }

    fn read_block(&self, sector: usize, dest: &mut [MaybeUninit<u8>]) -> Option<usize> {
        self.dev.read_block(sector, dest)
    }

    fn write_block(&self, sector: usize, buf: &[u8]) -> Option<usize> {
        self.dev.write_block(sector, buf)
    }
}

impl CachedAccess for BlockDevice {
    fn sref(&self) -> Weak<dyn CachedAccess> {
        self.sref.clone()
    }
}

impl INodeInterface for BlockDevice {}

impl Device for BlockDevice {
    fn device_marker(&self) -> usize {
        self.id
    }

    fn device_name(&self) -> String {
        self.name()
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        self.sref.upgrade().unwrap().clone()
    }
}

struct PartitionBlockDevice {
    sref: Weak<Self>,

    offset: usize, // offset in sectors
    size: usize,   // capacity in sectors
    device: Arc<dyn BlockDeviceInterface>,
}

impl PartitionBlockDevice {
    fn new(offset: usize, size: usize, device: Arc<dyn BlockDeviceInterface>) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            offset,
            size,
            device,
        })
    }
}

impl BlockDeviceInterface for PartitionBlockDevice {
    fn read_block(&self, sector: usize, dest: &mut [MaybeUninit<u8>]) -> Option<usize> {
        if sector >= self.size {
            return None;
        }

        self.device.read_block(self.offset + sector, dest)
    }

    fn block_size(&self) -> usize {
        self.device.block_size()
    }

    fn write_block(&self, sector: usize, buf: &[u8]) -> Option<usize> {
        if sector >= self.size {
            return None;
        }

        self.device.write_block(self.offset + sector, buf)
    }
}

impl CachedAccess for PartitionBlockDevice {
    fn sref(&self) -> Weak<dyn CachedAccess> {
        self.sref.clone()
    }
}

pub fn launch() -> Result<()> {
    let mut blocks_copy = Vec::<Arc<BlockDevice>>::new();

    for (_, device) in BLOCK_DEVS.lock().iter() {
        blocks_copy.push(device.clone());
    }

    for block in blocks_copy {
        if let Some(gpt) = Gpt::new(block.clone()) {
            log::info!("block: found GPT on {}!", block.name());

            for (i, entry) in gpt
                .entries()
                .iter()
                .enumerate()
                .filter(|(_, e)| e.is_used())
            {
                let start = entry.start_lba() as usize;
                let size = entry.size() as usize;

                log::info!(
                    "gpt: found partition (name=`{}`, start={:#x}, size{:#x})!",
                    entry.partition_name(),
                    start,
                    size
                );

                let name = alloc::format!("{}p{}", block.name(), i);
                let partition_device = PartitionBlockDevice::new(start, size, block.clone());
                let device = BlockDevice::new(name, partition_device);

                install_block_device(device.clone())?;

                // Check what filesystem is on this partition and mount it.
                if let Some(ext2) = Ext2::new(device.clone()) {
                    log::info!("gpt: found ext2 filesystem on {}!", device.name());

                    super::ROOT_FS.call_once(|| ext2.clone());
                    super::ROOT_DIR.call_once(|| ext2.root_dir().clone());
                }
            }
        }
    }

    super::devfs::init()?;
    log::info!("installed devfs");

    super::procfs::init()?;
    log::info!("installed procfs");

    Ok(())
}
