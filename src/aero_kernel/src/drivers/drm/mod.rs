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

mod rawfb;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use bit_field::BitField;

use crate::fs;
use crate::fs::devfs;
use crate::fs::inode::INodeInterface;
use crate::fs::FileSystemError;

use crate::mem::paging::VirtAddr;
use crate::utils::sync::Mutex;

use uapi::drm::*;

/// Represents modset objects visible to userspace; this includes connectors,
/// CRTCs, encoders, frambuffers and planes.
trait ModeObject: Send + Sync {
    /// Returns the mode object's ID.
    fn id(&self) -> u32;
}

trait DrmDevice: Send + Sync {
    /// Returns weather the DRM device supports creating dumb buffers.
    fn dumb_create(&self) -> bool;

    /// Returns tuple containing the minumum dimensions (`xmin`, `ymin`).
    fn min_dim(&self) -> (usize, usize);
    /// Returns tuple containing the miximum dimensions (`xmax`, `ymax`).
    fn max_dim(&self) -> (usize, usize);

    /// Returns a tuple containg the driver major, minor and patch level respectively.
    fn driver_version(&self) -> (usize, usize, usize);
    /// Returns a tuple contaning the driver name, desc and date respectively.
    fn driver_info(&self) -> (&'static str, &'static str, &'static str);
}

// ## Notes:
//
// Plane: Image source
//      - one or more framebuffers.
//      - contains cropped/resized version of the framebuffer.
//
// CRTCs:
//      - takes the plane and performs the composition.
//      - has the display mode and params.
//
// Encoders:
//      - takes the raw data from the CRTCs and converts it into a
//        specific format.
//
// Connectors:
//      - outputs the encoded data to an external display.
//      - handles hotplug events.
//      - reads EDIDs.
//
// Plane -> CRTCs -> Encoder -> Connector
//                |============ LCD connector

#[derive(Default)]
struct Crtc {
    id: u32,
}

impl ModeObject for Crtc {
    fn id(&self) -> u32 {
        self.id
    }
}

#[derive(Default)]
struct Encoder {
    id: u32,
}

impl ModeObject for Encoder {
    fn id(&self) -> u32 {
        self.id
    }
}

/// Represents a display connector; transmits the signal to the display, detects
/// display connection, removal and exposes the display's supported modes.
#[derive(Default)]
struct Connector {
    id: u32,
}

impl ModeObject for Connector {
    fn id(&self) -> u32 {
        self.id
    }
}

/// Holds information in relation to the framebuffer; this includes the
/// size and pixel format.
#[derive(Default)]
struct Framebuffer {
    id: u32,
}

impl ModeObject for Framebuffer {
    fn id(&self) -> u32 {
        self.id
    }
}

fn copy_field<T>(buffer: *mut T, buffer_size: &mut usize, value: &[T]) {
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

    // All of the mode objects:
    crtcs: Mutex<Vec<Crtc>>,
    encoders: Mutex<Vec<Encoder>>,
    connectors: Mutex<Vec<Connector>>,
    framebuffers: Mutex<Vec<Framebuffer>>,
}

impl Drm {
    pub fn new(device: Arc<dyn DrmDevice>) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            inode: devfs::alloc_device_marker(),
            card_id: DRM_CARD_ID.fetch_add(1, Ordering::SeqCst),
            device,

            crtcs: Mutex::new(alloc::vec![]),
            encoders: Mutex::new(alloc::vec![]),
            connectors: Mutex::new(alloc::vec![]),
            framebuffers: Mutex::new(alloc::vec![]),
        })
    }

    /// Installs and initializes the CRTC identifier.
    pub fn install_crtc(&self, mut crtc: Crtc) {
        let mut crtcs = self.crtcs.lock();

        crtc.id = crtcs.len() as u32;
        crtcs.push(crtc);
    }

    /// Installs and initializes the connector identifier.
    pub fn install_connector(&self, mut connector: Connector) {
        let mut connectors = self.connectors.lock();

        connector.id = connectors.len() as u32;
        connectors.push(connector);
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

                copy_field::<u8>(struc.name, &mut struc.name_len, name.as_bytes());
                copy_field::<u8>(struc.desc, &mut struc.desc_len, desc.as_bytes());
                copy_field::<u8>(struc.date, &mut struc.date_len, date.as_bytes());

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

            DRM_IOCTL_MODE_GETRESOURCES => {
                let struc = VirtAddr::new(arg as u64)
                    .read_mut::<DrmModeCardRes>()
                    .unwrap();

                /// Copies the mode object IDs into the user provided buffer. For saftey, checkout
                /// the [`copy_field`] function.
                fn copy_mode_obj_id<T: ModeObject>(
                    obj: &Mutex<Vec<T>>,
                    buffer: *mut u32,
                    buffer_size: &mut u32,
                ) {
                    let objs = obj.lock();
                    let mut count_objs = 0;

                    copy_field::<u32>(
                        buffer,
                        &mut count_objs,
                        objs.iter().map(|e| e.id()).collect::<Vec<_>>().as_slice(),
                    );

                    *buffer_size = count_objs as _;
                }

                let crtc_id_ptr = struc.crtc_id_ptr as *mut u32;
                let encoder_id_ptr = struc.encoder_id_ptr as *mut u32;
                let con_id_ptr = struc.connector_id_ptr as *mut u32;
                let fb_id_ptr = struc.fb_id_ptr as *mut u32;

                copy_mode_obj_id(&self.crtcs, crtc_id_ptr, &mut struc.count_crtcs);
                copy_mode_obj_id(&self.encoders, encoder_id_ptr, &mut struc.count_encoders);
                copy_mode_obj_id(&self.connectors, con_id_ptr, &mut struc.count_connectors);
                copy_mode_obj_id(&self.framebuffers, fb_id_ptr, &mut struc.count_fbs);

                let (xmin, ymin) = self.device.min_dim();

                struc.min_width = xmin as _;
                struc.min_height = ymin as _;

                let (xmax, ymax) = self.device.max_dim();

                struc.max_width = xmax as _;
                struc.max_height = ymax as _;

                Ok(0)
            }

            DRM_IOCTL_GET_CONNECTOR => {
                let struc = VirtAddr::new(arg as u64)
                    .read_mut::<DrmModeGetConnector>()
                    .unwrap();

                Ok(0)
            }

            _ => {
                // command[8..16] is the ASCII character supposedly unique to each driver.
                if command.get_bits(8..16) == DRM_IOCTL_BASE {
                    // command[0..8] is the function number.
                    let function = command.get_bits(0..8);
                    unimplemented!("drm: function (`{function:#x}`) not supported")
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
