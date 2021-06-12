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

use core::mem;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use spin::{Once, RwLock};

use crate::fs::cache::INODE_CACHE;

use super::inode::INodeInterface;
use super::{FileSystem, FilesystemError, Result};

static DEVICES: RwLock<BTreeMap<usize, Arc<dyn Device>>> = RwLock::new(BTreeMap::new());

static DEV_STDOUT: Once<Arc<DevStdout>> = Once::new();
static DEV_NULL: Once<Arc<DevNull>> = Once::new();

pub trait Device: Send + Sync {
    fn signature(&self) -> usize;
}

fn install_device(device: Arc<dyn Device>) -> Result<()> {
    let dev = DEVICES.read();

    if dev.contains_key(&device.signature()) {
        Err(FilesystemError::DeviceExists)
    } else {
        mem::drop(dev);

        DEVICES.write().insert(device.signature(), device);
        Ok(())
    }
}

pub struct DevFs;

impl FileSystem for DevFs {}

macro impl_dev($(struct $name:ty;)*) {
    $(
        impl Device for $name {
            fn signature(&self) -> usize {
                self.0
            }
        }
    )*
}

pub struct DevNull(usize);

impl INodeInterface for DevNull {
    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> Result<usize> {
        Ok(0x00)
    }

    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> Result<usize> {
        Ok(0x00)
    }
}

pub struct DevStdout(usize);

impl INodeInterface for DevStdout {
    fn write_at(&self, _offset: usize, buffer: &[u8]) -> Result<usize> {
        let string = unsafe { core::str::from_utf8_unchecked(buffer) };

        log::debug!("(stdout) {}", string);
        Ok(buffer.len())
    }

    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> Result<usize> {
        Err(FilesystemError::NotSupported)
    }
}

impl_dev! {
    struct DevNull;
    struct DevStdout;
}

pub fn get_stdout() -> &'static Arc<DevStdout> {
    DEV_STDOUT
        .get()
        .expect("Attempted to get /dev/stdout before it was initialized")
}

pub fn get_null() -> &'static Arc<DevNull> {
    DEV_NULL
        .get()
        .expect("Attempted to get /dev/null before it was initialized")
}

/// Initialize devfs and install it in the dyn filesystem btreemap.
pub(super) fn init() -> Result<()> {
    let _ = INODE_CACHE
        .get()
        .expect("INode cache was not even initialized");

    let devfs = DevFs;

    {
        DEV_NULL.call_once(|| Arc::new(DevNull(0x6e756c6c)));
        DEV_STDOUT.call_once(|| Arc::new(DevStdout(0x7374646f7574)));

        install_device(get_null().clone())?;
        log::debug!("Installed /dev/null");

        install_device(get_stdout().clone())?;
        log::debug!("Installed /dev/stdout");
    }

    /*
     * Now after we have initialized devfs we are going to install it as a filesystem
     * in our dyn filesystems hashmap with `0x646576` as its signature.
     */
    super::install_filesystem(0x646576, devfs)?;

    log::debug!("Installed devfs");

    Ok(())
}
