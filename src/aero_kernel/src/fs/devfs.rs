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

//! The `/dev` directory contains the special device files for all the devices.

use core::mem;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;

use spin::{Once, RwLock};

use crate::fs::lookup_path;
use crate::fs::Path;
use crate::logger;

use super::cache::DirCacheItem;
use super::inode::INodeInterface;
use super::ramfs::RamFs;
use super::FileSystemError;
use super::{FileSystem, Result, MOUNT_MANAGER};

lazy_static::lazy_static! {
    static ref DEV_FILESYSTEM: Arc<DevFs> = DevFs::new();
}

static DEVICES: RwLock<BTreeMap<usize, Arc<dyn Device>>> = RwLock::new(BTreeMap::new());
static DEVICE_MARKER: AtomicUsize = AtomicUsize::new(0x00);

pub fn alloc_device_marker() -> usize {
    DEVICE_MARKER.fetch_add(1, Ordering::SeqCst)
}

/// A trait representing a device. A device has a device marker (or a device ID) and the
/// device name (which is used in the creation of the device inode in the device filesystem).
pub trait Device: Send + Sync {
    /// Returns the device marker (or simply the device ID) of the device. (See the documentation of
    /// this trait for more information.)
    fn device_marker(&self) -> usize;

    /// Returns the device name of this device. (See the documentation of this trait for more
    /// information.)
    fn device_name(&self) -> String;
    fn inode(&self) -> Arc<dyn INodeInterface>;
}

/// Installs the provided `device` in the device filesystem (ie. in /dev/) and the
/// global [DEVICES] btree map.
pub fn install_device(device: Arc<dyn Device>) -> Result<()> {
    let devices = DEVICES.read();

    let device_marker = device.device_marker();
    let device_name = device.device_name();

    // We cannot have two devices with the same device marker.
    if devices.contains_key(&device_marker) {
        return Err(FileSystemError::EntryExists);
    }

    mem::drop(devices);

    DEVICES.write().insert(device_marker, device.clone());

    DEV_FILESYSTEM
        .root_dir()
        .inode()
        .make_dev_inode(&device_name, device_marker)?;

    log::debug!("installed device `{}`", device_name);

    Ok(())
}

/// Structure representing a device inode. This is internally used by ram-fs
/// to create a new inode with the file type of `device` and its contents as a
/// reference-counting pointer to the device itself.
pub struct DevINode(Arc<dyn Device>);

impl DevINode {
    /// Creates a new device inode by looking up the device with the provided `marker`
    /// as the key in the [DEVICES] b-tree map.
    pub fn new(marker: usize) -> Result<Arc<Self>> {
        let this = DEVICES.read();

        if let Some(device) = this.get(&marker) {
            Ok(Arc::new(Self(device.clone())))
        } else {
            Err(FileSystemError::EntryNotFound)
        }
    }
}

impl INodeInterface for DevINode {
    fn write_at(&self, offset: usize, buffer: &[u8]) -> Result<usize> {
        self.0.inode().write_at(offset, buffer)
    }

    fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        self.0.inode().read_at(offset, buffer)
    }

    fn ioctl(&self, command: usize, arg: usize) -> Result<usize> {
        self.0.inode().ioctl(command, arg)
    }
}

/// Implementation of dev filesystem. (See the module-level documentation for more
/// information).
struct DevFs(Arc<RamFs>);

impl DevFs {
    fn new() -> Arc<Self> {
        Arc::new(Self(RamFs::new()))
    }
}

impl FileSystem for DevFs {
    fn root_dir(&self) -> DirCacheItem {
        self.0.root_dir()
    }
}

/// Implementation of the null device (akin `/dev/null`).
struct DevNull(usize);

impl DevNull {
    fn new() -> Arc<Self> {
        Arc::new(Self(alloc_device_marker()))
    }
}

impl Device for DevNull {
    fn device_marker(&self) -> usize {
        self.0
    }

    fn device_name(&self) -> String {
        String::from("null")
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        DEV_NULL.get().expect("device not initialized").clone()
    }
}

impl INodeInterface for DevNull {
    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> Result<usize> {
        Ok(0x00)
    }

    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> Result<usize> {
        Ok(0x00)
    }
}

struct DevKmsg(usize);

impl DevKmsg {
    fn new() -> Arc<Self> {
        Arc::new(Self(alloc_device_marker()))
    }
}

impl Device for DevKmsg {
    fn device_marker(&self) -> usize {
        self.0
    }

    fn device_name(&self) -> String {
        String::from("kmsg")
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        DEV_KMSG.get().expect("device not initialized").clone()
    }
}

impl INodeInterface for DevKmsg {
    fn read_at(&self, offset: usize, buffer: &mut [u8]) -> Result<usize> {
        let buf = logger::get_log_buffer();

        let size = core::cmp::min(buffer.len(), buf.len());
        buffer[..size].copy_from_slice(&buf.as_bytes()[offset..offset + size]);

        Ok(size)
    }

    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> Result<usize> {
        Ok(0x00)
    }
}

struct DevFb(usize);

impl DevFb {
    fn new() -> Arc<Self> {
        Arc::new(Self(alloc_device_marker()))
    }
}

impl Device for DevFb {
    fn device_marker(&self) -> usize {
        self.0
    }

    fn device_name(&self) -> String {
        String::from("fb")
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        DEV_FB.get().expect("device not initialized").clone()
    }
}

impl INodeInterface for DevFb {
    fn write_at(&self, offset: usize, buffer: &[u8]) -> Result<usize> {
        crate::rendy::DEBUG_RENDY
            .get()
            .map(|e| {
                let mut lock = e.lock_irq();
                let fb = lock.get_framebuffer();

                let mut count = buffer.len();

                if offset + buffer.len() > fb.len() {
                    count = buffer.len() - ((offset + buffer.len()) - fb.len());
                }

                let raw = buffer.as_ptr() as *const u32;
                let src = unsafe { core::slice::from_raw_parts(raw, count) };

                fb[offset..offset + count].copy_from_slice(src);
                Ok(count)
            })
            .expect("/dev/fb: terminal not initialized")
    }
}

struct DevUrandom(usize);

impl DevUrandom {
    fn new() -> Arc<Self> {
        Arc::new(Self(alloc_device_marker()))
    }
}

impl Device for DevUrandom {
    fn device_marker(&self) -> usize {
        self.0
    }

    fn device_name(&self) -> String {
        String::from("urandom")
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        DEV_URANDOM.get().expect("device not initialized").clone()
    }
}

impl INodeInterface for DevUrandom {
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> Result<usize> {
        for (_, b) in buffer.iter_mut().enumerate() {
            *b = 0;
        }

        Ok(buffer.len())
    }
}

static DEV_NULL: Once<Arc<DevNull>> = Once::new();
static DEV_KMSG: Once<Arc<DevKmsg>> = Once::new();
static DEV_FB: Once<Arc<DevFb>> = Once::new();
static DEV_URANDOM: Once<Arc<DevUrandom>> = Once::new();

/// Initializes the dev filesystem. (See the module-level documentation for more information).
pub(super) fn init() -> Result<()> {
    lazy_static::initialize(&DEV_FILESYSTEM);

    let inode = lookup_path(Path::new("/dev"))?;
    MOUNT_MANAGER.mount(inode, DEV_FILESYSTEM.clone())?;

    {
        let null = DEV_NULL.call_once(|| DevNull::new());
        let kmsg = DEV_KMSG.call_once(|| DevKmsg::new());
        let fb = DEV_FB.call_once(|| DevFb::new());
        let urandom = DEV_URANDOM.call_once(|| DevUrandom::new());

        install_device(null.clone())?;
        install_device(kmsg.clone())?;
        install_device(fb.clone())?;
        install_device(urandom.clone())?;
    }

    Ok(())
}
