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

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use spin::RwLock;

use super::{AeroFilesystemError, FileSystem};

static DEVICES: RwLock<BTreeMap<usize, Arc<dyn Device>>> = RwLock::new(BTreeMap::new());

pub trait Device: Send + Sync {
    fn signature(&self) -> usize;
}

macro impl_dev() {
    fn signature(&self) -> usize {
        Self::SIGNATURE
    }
}

pub struct DevFs;

impl DevFs {
    pub const SIGNATURE: usize = 0x646576;
}

impl FileSystem for DevFs {}

struct DevNull;
struct DevZero;

impl DevNull {
    pub const SIGNATURE: usize = 0x6e756c6c;
}

impl DevZero {
    pub const SIGNATURE: usize = 0x7a65726f;
}

impl Device for DevNull {
    impl_dev!();
}

impl Device for DevZero {
    impl_dev!();
}

pub(super) fn install_device<D: 'static + Device>(
    signature: usize,
    device: D,
) -> Result<(), AeroFilesystemError> {
    let dev = DEVICES.read();

    if dev.contains_key(&signature) {
        Err(AeroFilesystemError::DeviceExists)
    } else {
        drop(dev);
        DEVICES.write().insert(signature, Arc::new(device));

        Ok(())
    }
}

/// Initialize devfs and install it in the dyn filesystem btreemap.
pub(super) fn init() -> Result<(), AeroFilesystemError> {
    let devfs = DevFs;

    {
        install_device(DevNull::SIGNATURE, DevNull)?;
        log::debug!("Installed /dev/null");

        install_device(DevZero::SIGNATURE, DevZero)?;
        log::debug!("Installed /dev/zero");
    }

    /*
     * Now after we have initialized devfs we are going to install it as a filesystem
     * in our dyn filesystems hashmap with `0x646576` as its signature.
     */
    super::install_filesystem(DevFs::SIGNATURE, devfs)?;

    log::debug!("Installed devfs");

    Ok(())
}
