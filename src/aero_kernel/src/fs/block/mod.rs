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

use gpt::Gpt;

use core::mem::MaybeUninit;

use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::fs::devfs::install_device;
use crate::fs::{FileSystem, Result};

use crate::fs::ext2::Ext2;
use crate::mem::paging::*;
use crate::utils::sync::Mutex;

use super::cache::{Cache, CacheArc, CacheItem, Cacheable};
use super::devfs::{alloc_device_marker, Device};
use super::inode::INodeInterface;

type PageCacheKey = (usize, usize); // (block device pointer, offset)
type PageCacheItem = CacheArc<CacheItem<PageCacheKey, CachedPage>>;

struct CachedPage {
    device: Weak<dyn CachedAccess>,
    offset: usize,
    page: PhysFrame,
}

impl CachedPage {
    fn new(device: Weak<dyn CachedAccess>, offset: usize) -> Self {
        Self {
            device,
            offset,
            page: FRAME_ALLOCATOR
                .allocate_frame()
                .expect("page_cache: out of memory"),
        }
    }

    fn data_mut(&self) -> &mut [MaybeUninit<u8>] {
        let data_ptr = self
            .page
            .start_address()
            .as_hhdm_virt()
            .as_mut_ptr::<MaybeUninit<u8>>();

        // SAFETY: It is safe to create a slice of MaybeUninit<T> because it has the same
        // size and alignment as T.
        unsafe { core::slice::from_raw_parts_mut(data_ptr, Size4KiB::SIZE as usize) }
    }

    fn data_addr(&self) -> PhysAddr {
        self.page.start_address()
    }

    fn make_key(device: Weak<dyn CachedAccess>, offset: usize) -> PageCacheKey {
        (device.as_ptr() as *const u8 as usize, offset)
    }
}

impl Cacheable<PageCacheKey> for CachedPage {
    fn cache_key(&self) -> PageCacheKey {
        Self::make_key(self.device.clone(), self.offset)
    }
}

lazy_static::lazy_static! {
    static ref PAGE_CACHE: Arc<Cache<PageCacheKey, CachedPage>> = Cache::new();
}

impl Cache<PageCacheKey, CachedPage> {
    /// Returns the cached page at the given offset, if not present, it will be allocated,
    /// initialized with the data on the disk and placed in the page cache.
    ///
    /// ## Arguments
    ///
    /// * `device` - The device to get the page from.
    ///
    /// * `offset` - The offset in bytes to the data. This will be rounded down to
    ///              the nearest page boundary.
    pub fn get_page(&self, device: Weak<dyn CachedAccess>, offset: usize) -> PageCacheItem {
        let cache_offset = offset / Size4KiB::SIZE as usize;
        let cache_key = CachedPage::make_key(device.clone(), cache_offset);

        if let Some(page) = PAGE_CACHE.get(cache_key) {
            return page;
        }

        let page = CachedPage::new(device.clone(), cache_offset);
        let device = device.upgrade().expect("page_cache: device dropped");

        let aligned_offset = align_down(offset as u64, Size4KiB::SIZE) as usize;
        let sector = aligned_offset / device.block_size();

        device
            .read_dma(sector, page.data_addr(), Size4KiB::SIZE as usize)
            .expect("page_cache: failed to read block");

        PAGE_CACHE.make_item_cached(page)
    }
}

pub trait BlockDeviceInterface: Send + Sync {
    fn block_size(&self) -> usize;

    fn read_dma(&self, sector: usize, start: PhysAddr, size: usize) -> Option<usize>;

    fn read_block(&self, sector: usize, dest: &mut [MaybeUninit<u8>]) -> Option<usize>;
    fn write_block(&self, sector: usize, buf: &[u8]) -> Option<usize>;
}

pub trait CachedAccess: BlockDeviceInterface {
    fn sref(&self) -> Weak<dyn CachedAccess>;

    fn read(&self, mut offset: usize, dest: &mut [MaybeUninit<u8>]) -> Option<usize> {
        let mut loc = 0;

        while loc < dest.len() {
            let page = PAGE_CACHE.get_page(self.sref(), offset);

            let page_offset = offset % Size4KiB::SIZE as usize;
            let size = core::cmp::min(Size4KiB::SIZE as usize - page_offset, dest.len() - loc);

            let data = &page.data_mut()[page_offset..page_offset + size];
            dest[loc..loc + size].copy_from_slice(data);

            core::mem::forget(page);

            loc += size;
            offset = align_down(offset as u64 + Size4KiB::SIZE, Size4KiB::SIZE) as usize;
        }

        Some(loc)
    }

    /// Writes the given data to the device at the given offset and returns the
    /// number of bytes written.
    ///
    /// ## Notes
    ///
    /// * This function does **not** sync the written data to the disk.
    fn write(&self, mut offset: usize, buffer: &[u8]) -> Option<usize> {
        let mut loc = 0;

        while loc < buffer.len() {
            let page = PAGE_CACHE.get_page(self.sref(), offset);

            let page_offset = offset % Size4KiB::SIZE as usize;
            let size = core::cmp::min(Size4KiB::SIZE as usize - page_offset, buffer.len() - loc);

            MaybeUninit::write_slice(
                &mut page.data_mut()[page_offset..page_offset + size],
                &buffer[loc..loc + size],
            );

            core::mem::forget(page);

            loc += size;
            offset = align_down(offset as u64 + Size4KiB::SIZE, Size4KiB::SIZE) as usize;
        }

        Some(loc)
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

    fn read_dma(&self, sector: usize, start: PhysAddr, size: usize) -> Option<usize> {
        self.dev.read_dma(sector, start, size)
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

    fn write_block(&self, sector: usize, buf: &[u8]) -> Option<usize> {
        if sector >= self.size {
            return None;
        }

        self.device.write_block(self.offset + sector, buf)
    }

    fn read_dma(&self, sector: usize, start: PhysAddr, size: usize) -> Option<usize> {
        if sector >= self.size {
            return None;
        }

        self.device.read_dma(self.offset + sector, start, size)
    }

    fn block_size(&self) -> usize {
        self.device.block_size()
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
