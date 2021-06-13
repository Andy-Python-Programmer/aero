/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

//! The `/dev` directory contains the special device files for all the devices.

use core::mem;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use spin::RwLock;

use crate::fs::lookup_path;
use crate::fs::Path;

use super::cache::DirCacheItem;
use super::ramfs::RamFs;
use super::FileSystemError;
use super::{FileSystem, Result, MOUNT_MANAGER};

lazy_static::lazy_static! {
    static ref DEV_FILESYSTEM: Arc<DevFs> = DevFs::new();
}

static DEVICES: RwLock<BTreeMap<usize, Arc<dyn Device>>> = RwLock::new(BTreeMap::new());
static DEVICE_MARKER: AtomicUsize = AtomicUsize::new(0x00);

/// A trait representing a device. A device has a device marker (or a device ID) and the
/// device name (which is used in the creation of the device inode in the device filesystem).
trait Device: Send + Sync {
    /// Returns the device marker (or simply the device ID) of the device. (See the documentation of
    /// this trait for more information.)
    fn device_marker(&self) -> usize;

    /// Returns the device name of this device. (See the documentation of this trait for more
    /// information.)
    fn device_name(&self) -> &str;
}

/// Installs the provided `device` in the device filesystem (ie. in /dev/) and the
/// global [DEVICES] btree map.
fn install_device(device: Arc<dyn Device>) -> Result<()> {
    let devices = DEVICES.read();

    // We cannot have two devices with the same device marker.
    if devices.contains_key(&device.device_marker()) {
        return Err(FileSystemError::EntryExists);
    }

    mem::drop(devices);

    DEVICES
        .write()
        .insert(device.device_marker(), device.clone());

    log::debug!("Installed device `{}`", device.device_name());

    Ok(())
}

struct DevFs(Arc<RamFs>);

impl DevFs {
    #[inline]
    fn new() -> Arc<Self> {
        Arc::new(Self(RamFs::new()))
    }
}

impl FileSystem for DevFs {
    #[inline]
    fn root_dir(&self) -> DirCacheItem {
        self.0.root_dir()
    }
}

/// Implementation of the null device (akin `/dev/null`).
struct DevNull(usize);

impl DevNull {
    #[inline]
    fn new() -> Arc<Self> {
        Arc::new(Self(DEVICE_MARKER.fetch_add(1, Ordering::SeqCst)))
    }
}

impl Device for DevNull {
    #[inline]
    fn device_marker(&self) -> usize {
        self.0
    }

    #[inline]
    fn device_name(&self) -> &str {
        "null"
    }
}

/// Implementation of the stdout device (akin `/dev/stdout`).
struct DevStdout(usize);

impl DevStdout {
    #[inline]
    fn new() -> Arc<Self> {
        Arc::new(Self(DEVICE_MARKER.fetch_add(1, Ordering::SeqCst)))
    }
}

impl Device for DevStdout {
    #[inline]
    fn device_marker(&self) -> usize {
        self.0
    }

    #[inline]
    fn device_name(&self) -> &str {
        "stdout"
    }
}

/// Initializes the dev filesystem. (See the module-level documentation for more information).
pub(super) fn init() -> Result<()> {
    let inode = lookup_path(Path::new("/dev"))?;

    MOUNT_MANAGER.mount(inode, DEV_FILESYSTEM.clone())?;

    {
        let null = DevNull::new();
        let stdout = DevStdout::new();

        install_device(null)?;
        install_device(stdout)?;
    }

    Ok(())
}
