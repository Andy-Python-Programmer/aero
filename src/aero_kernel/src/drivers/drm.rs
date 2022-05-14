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

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::string::String;
use alloc::sync::{Arc, Weak};
use bit_field::BitField;

use crate::fs::devfs;
use crate::fs::inode::INodeInterface;
use crate::fs::FileSystem;
use crate::fs::{self, FileSystemError};

use crate::mem::paging::VirtAddr;

use uapi::drm::*;

trait DrmDevice: Send + Sync {
    /// Returns weather the DRM device supports creating dumb buffers.
    fn dumb_create(&self) -> bool;

    /// Returns a tuple containg the driver major, minor and patch level respectively.
    fn driver_version(&self) -> (usize, usize, usize);
    /// Returns a tuple contaning the driver name, desc and date respectively.
    fn driver_info(&self) -> (&'static str, &'static str, &'static str);
}

fn copy_field(buffer: *mut u8, buffer_size: &mut usize, value: &[u8]) {
    // do not overflow the user buffer.
    let mut copy_len = value.len();

    if *buffer_size > value.len() {
        copy_len = *buffer_size;
    }

    // let userspace know exact length of driver value (which could be
    // larger than the userspace-supplied buffer).
    *buffer_size = value.len();

    // finally, try filling in the user buffer.
    if copy_len != 0 && !buffer.is_null() {
        unsafe {
            core::ptr::copy_nonoverlapping(value.as_ptr(), buffer, copy_len);
        }
    }
}

static DRM_CARD_ID: AtomicUsize = AtomicUsize::new(0);

/// The direct rendering manager (DRM) exposes the GPUs through the device filesystem. Each
/// GPU detected by the DRM is referred to as a DRM device and a device file (`/dev/dri/cardX`)
/// is created to interface with it; where X is a sequential number.
struct Drm {
    sref: Weak<Self>,

    inode: usize,
    card_id: usize,
    device: Arc<dyn DrmDevice>,
}

impl Drm {
    pub fn new(device: Arc<dyn DrmDevice>) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            inode: devfs::alloc_device_marker(),
            card_id: DRM_CARD_ID.fetch_add(1, Ordering::SeqCst),
            device,
        })
    }
}

impl INodeInterface for Drm {
    // The DRM is accessed using IOCTLs on a device representing a graphics
    // card.
    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            DRM_IOCTL_VERSION => {
                let struc = VirtAddr::new(arg as u64).read_mut::<DrmVersion>().unwrap();

                let (major, minor, patch_level) = self.device.driver_version();
                let (name, desc, date) = self.device.driver_info();

                struc.version_major = major as _;
                struc.version_minor = minor as _;
                struc.version_patch_level = patch_level as _;

                copy_field(struc.name, &mut struc.name_len, name.as_bytes());
                copy_field(struc.desc, &mut struc.desc_len, desc.as_bytes());
                copy_field(struc.date, &mut struc.date_len, date.as_bytes());

                Ok(0)
            }

            DRM_IOCTL_GET_CAP => {
                let struc = VirtAddr::new(arg as u64).read_mut::<DrmGetCap>().unwrap();

                // NOTE: The user is responsible for zeroing out the structure.
                match struc.capability {
                    DRM_CAP_DUMB_BUFFER => {
                        if self.device.dumb_create() {
                            struc.value = 1;
                        }
                    }

                    cap => {
                        log::warn!("drm: unknown capability (`{cap}`)");
                        return Err(FileSystemError::NotSupported);
                    }
                }

                Ok(0)
            }

            _ => {
                // command[8..16] is the ASCII character supposedly unique to each driver.
                if command.get_bits(8..16) == DRM_IOCTL_BASE {
                    // command[0..8] is the function number.
                    let function = command.get_bits(0..8);
                    unimplemented!("drm: function (`{function}`) not supported")
                }

                log::warn!("drm: unknown ioctl command (`{command}`)");
                Err(FileSystemError::NotSupported)
            }
        }
    }
}

impl devfs::Device for Drm {
    fn device_marker(&self) -> usize {
        self.inode
    }

    fn device_name(&self) -> String {
        alloc::format!("card{}", self.card_id) // `/dev/dri/cardX`
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        self.sref.upgrade().unwrap()
    }
}

struct RawFramebuffer {}

impl DrmDevice for RawFramebuffer {
    fn driver_version(&self) -> (usize, usize, usize) {
        (0, 0, 1)
    }

    fn driver_info(&self) -> (&'static str, &'static str, &'static str) {
        ("rawfb_gpu", "rawfb gpu", "0")
    }

    fn dumb_create(&self) -> bool {
        true
    }
}

fn init() {
    let dri = devfs::DEV_FILESYSTEM
        .root_dir()
        .inode()
        .mkdir("dri")
        .expect("devfs: failed to create DRM directory");

    let rfb = Drm::new(Arc::new(RawFramebuffer {}));
    devfs::install_device_at(dri, rfb).expect("ramfs: failed to install DRM device");
}

crate::module_init!(init);
