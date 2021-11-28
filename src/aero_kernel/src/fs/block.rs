use core::alloc::Layout;

use alloc::alloc::alloc_zeroed;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::fs::devfs::install_device;
use crate::fs::Result;

use crate::mem::paging::VirtAddr;
use crate::utils::sync::Mutex;

use super::devfs::{alloc_device_marker, Device};
use super::inode::INodeInterface;

pub trait BlockDeviceInterface: Send + Sync {
    fn read(&self, sector: usize, dest: &mut [u8]) -> Option<usize>;
    fn write(&self, sector: usize, buf: &[u8]) -> Option<usize>;
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
    self_ref: Weak<BlockDevice>,
}

impl BlockDevice {
    pub fn new(name: String, imp: Arc<dyn BlockDeviceInterface>) -> Arc<BlockDevice> {
        Arc::new_cyclic(|me| BlockDevice {
            id: alloc_device_marker(),
            name,
            dev: imp,
            self_ref: me.clone(),
        })
    }

    pub fn name(&self) -> String {
        self.name.clone()
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
        self.self_ref.upgrade().unwrap().clone()
    }
}

pub trait RawAccess: Send + Sync {
    fn read_direct(&self, addr: usize, dest: &mut [u8]) -> Option<usize>;
    fn write_direct(&self, addr: usize, buf: &[u8]) -> Option<usize>;
}

impl<T: RawAccess> BlockDeviceInterface for T {
    fn read(&self, sector: usize, dest: &mut [u8]) -> Option<usize> {
        self.read_direct(sector * 512, dest)
    }

    fn write(&self, sector: usize, buf: &[u8]) -> Option<usize> {
        self.write_direct(sector * 512, buf)
    }
}

impl RawAccess for BlockDevice {
    fn read_direct(&self, offset: usize, dest: &mut [u8]) -> Option<usize> {
        assert_eq!(offset % 512, 0);
        self.dev.read(offset / 512, dest)
    }

    fn write_direct(&self, offset: usize, buf: &[u8]) -> Option<usize> {
        assert_eq!(offset % 512, 0);
        self.dev.write(offset / 512, buf)
    }
}

pub struct Mbr {
    data: VirtAddr,
}

impl Mbr {
    pub fn new() -> Self {
        let layout = unsafe { Layout::from_size_align_unchecked(512, 0x1000) };
        let memory = unsafe { alloc_zeroed(layout) };

        Self {
            data: VirtAddr::new(memory as u64),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes_mut()
    }

    pub fn bytes_mut(&self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.data.as_mut_ptr(), 512) }
    }

    pub fn is_valid(&self) -> bool {
        self.bytes()[510..] == [0x55, 0xAA]
    }

    pub fn partition(&self, idx: usize) -> Option<Partition> {
        if idx >= 4 {
            return None;
        }
        let off = 0x01BEusize + 0x10 * idx;

        Some(Partition {
            data: &self.bytes()[off..off + 16],
        })
    }
}

pub struct Partition<'a> {
    data: &'a [u8],
}

impl Partition<'_> {
    pub fn total_sectors(&self) -> u32 {
        unsafe { *(self.data.as_ptr().offset(12) as *const u32) }
    }
}

pub fn launch() -> Result<()> {
    // NOTE: Copy all of the block devices into a vector since we will
    // need to lock the BLOCK_DEVS static when iterating which will
    // cause a deadlock.
    let mut blocks_copy = Vec::<Arc<BlockDevice>>::new();

    for (_, device) in BLOCK_DEVS.lock().iter() {
        blocks_copy.push(device.clone());
    }

    for block in blocks_copy {
        let mbr = Mbr::new();

        block.read(0, mbr.bytes_mut());
        log::debug!("{:?}", mbr.bytes_mut());

        if mbr.is_valid() {
            log::info!("{}: found MBR partition", block.name());

            for p in 0..4 {
                if let Some(part) = mbr.partition(p) {
                    if part.total_sectors() > 0 {
                        let name = alloc::format!("{}p{}", block.name(), p);
                        let block_device = BlockDevice::new(name, block.clone());

                        install_block_device(block_device)?;
                    }
                }
            }
        }
    }

    Ok(())
}
