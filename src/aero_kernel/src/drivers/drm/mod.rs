// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

mod rawfb;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use bit_field::BitField;
use hashbrown::HashMap;

use crate::arch::user_copy::UserRef;
use crate::fs;
use crate::fs::inode::INodeInterface;
use crate::fs::{devfs, FileSystemError};

use crate::mem::paging::*;
use crate::utils::sync::Mutex;

use uapi::drm::*;

/// Represents modset objects visible to userspace; this includes connectors,
/// CRTCs, encoders, frambuffers and planes.
#[downcastable]
trait ModeObject: Send + Sync {
    /// Returns the mode object's ID.
    fn id(&self) -> u32;
    fn object(&self) -> Arc<dyn ModeObject>;

    // Conversion methods:

    /// Converts this mode object into a connector.
    fn as_connector(&self) -> Option<Arc<Connector>> {
        self.object().downcast_arc::<Connector>()
    }

    /// Converts this mode object into an encoder.
    fn as_encoder(&self) -> Option<Arc<Encoder>> {
        self.object().downcast_arc::<Encoder>()
    }

    /// Converts this mode object into a CRTC.
    fn as_crtc(&self) -> Option<Arc<Crtc>> {
        self.object().downcast_arc::<Crtc>()
    }

    /// Converts this mode object into a framebuffer.
    fn as_framebuffer(&self) -> Option<Arc<Framebuffer>> {
        self.object().downcast_arc::<Framebuffer>()
    }
}

trait DrmDevice: Send + Sync {
    /// Returns weather the DRM device supports creating dumb buffers.
    fn can_dumb_create(&self) -> bool;

    fn dumb_create(&self, width: u32, height: u32, bpp: u32) -> (BufferObject, u32);
    fn framebuffer_create(&self, buffer_object: &BufferObject, width: u32, height: u32, pitch: u32);
    fn commit(&self, buffer_obj: &BufferObject);

    /// Returns tuple containing the minimum dimensions (`xmin`, `ymin`).
    fn min_dim(&self) -> (usize, usize);
    /// Returns tuple containing the maximum dimensions (`xmax`, `ymax`).
    fn max_dim(&self) -> (usize, usize);

    /// Returns a tuple containing the driver major, minor and patch level respectively.
    fn driver_version(&self) -> (usize, usize, usize);
    /// Returns a tuple containing the driver name, desc and date respectively.
    fn driver_info(&self) -> (&'static str, &'static str, &'static str);
}

#[derive(Debug, Clone)]
struct BufferObject {
    size: usize,
    mapping: usize,
    memory: Vec<PhysFrame>,
}

impl BufferObject {
    pub fn new(size: usize, memory: Vec<PhysFrame>) -> Self {
        Self {
            size,
            mapping: usize::MAX,
            memory,
        }
    }
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
//      - takes the raw data from the CRTCs and converts it into a specific format.
//
// Connectors:
//      - outputs the encoded data to an external display.
//      - handles hotplug events.
//      - reads EDIDs.
//
// Plane -> CRTCs -> Encoder -> Connector
//                |============ LCD connector

struct Crtc {
    sref: Weak<Self>,

    object_id: u32,
    index: u32,
}

impl Crtc {
    pub fn new(drm: &Drm, object_id: u32) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            object_id,
            index: drm.crtcs.lock().len() as _,
        })
    }
}

impl ModeObject for Crtc {
    fn id(&self) -> u32 {
        self.object_id
    }

    fn object(&self) -> Arc<dyn ModeObject> {
        self.sref.upgrade().unwrap()
    }
}

struct Encoder {
    sref: Weak<Self>,

    /// The current CRTC for this encoder.
    current_crtc: Arc<Crtc>,
    /// A vector containing all the possible CRTCs for this encoder.
    possible_crtcs: Vec<Arc<Crtc>>,
    /// A vector containing all the possible sibling encoders for cloning.
    possible_clones: Vec<Arc<Encoder>>,

    object_id: u32,
    index: u32,
}

impl Encoder {
    pub fn new(
        drm: &Drm,
        current_crtc: Arc<Crtc>,
        possible_crtcs: Vec<Arc<Crtc>>,
        object_id: u32,
    ) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            current_crtc,
            possible_crtcs,
            possible_clones: alloc::vec![], // todo: add self as possible clone.

            object_id,
            index: drm.encoders.lock().len() as _,
        })
    }
}

impl ModeObject for Encoder {
    fn id(&self) -> u32 {
        self.object_id
    }

    fn object(&self) -> Arc<dyn ModeObject> {
        self.sref.upgrade().unwrap()
    }
}

/// Represents a display connector; transmits the signal to the display, detects
/// display connection, removal and exposes the display's supported modes.
struct Connector {
    sref: Weak<Self>,

    /// The current status of the connector.
    status: DrmModeConStatus,
    /// The current encoder for this connector.
    current_encoder: Arc<Encoder>,
    /// A vector containing all the possible encoders for this connector.
    possible_encoders: Vec<Arc<Encoder>>,
    /// A vector containing all of the possible display modes for this connector.
    modes: Vec<DrmModeInfo>,

    connector_typ: u32,
    object_id: u32,
}

impl Connector {
    pub fn new(
        current_encoder: Arc<Encoder>,
        possible_encoders: Vec<Arc<Encoder>>,
        modes: Vec<DrmModeInfo>,
        status: DrmModeConStatus,
        object_id: u32,
    ) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            status,
            current_encoder,
            possible_encoders,
            modes,
            connector_typ: 0, // todo
            object_id,
        })
    }
}

impl ModeObject for Connector {
    fn id(&self) -> u32 {
        self.object_id
    }

    fn object(&self) -> Arc<dyn ModeObject> {
        self.sref.upgrade().unwrap()
    }
}

/// Holds information in relation to the framebuffer; this includes the
/// size and pixel format.
struct Framebuffer {
    sref: Weak<Self>,
    object_id: u32,
    buffer_obj: BufferObject, // todo: this should be a reference not a clone.
}

impl Framebuffer {
    pub fn new(object_id: u32, buffer_obj: BufferObject) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),
            object_id,
            buffer_obj,
        })
    }
}

impl ModeObject for Framebuffer {
    fn id(&self) -> u32 {
        self.object_id
    }

    fn object(&self) -> Arc<dyn ModeObject> {
        self.sref.upgrade().unwrap()
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

struct IdAllocator(AtomicUsize);

impl IdAllocator {
    pub fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    pub fn alloc(&self) -> usize {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

/// The direct rendering manager (DRM) exposes the GPUs through the device filesystem. Each
/// GPU detected by the DRM is referred to as a DRM device and a device file (`/dev/dri/cardX`)
/// is created to interface with it; where X is a sequential number.
struct Drm {
    sref: Weak<Self>,

    inode: usize,
    card_id: usize,
    device: Arc<dyn DrmDevice>,

    id_alloc: IdAllocator,
    mapping_alloc: IdAllocator,
    buffer_alloc: IdAllocator,

    buffers: Mutex<HashMap<u32, BufferObject>>,
    mode_objs: Mutex<HashMap<u32, Arc<dyn ModeObject>>>,

    // All of the mode objects:
    crtcs: Mutex<Vec<Arc<Crtc>>>,
    encoders: Mutex<Vec<Arc<Encoder>>>,
    connectors: Mutex<Vec<Arc<Connector>>>,
    framebuffers: Mutex<Vec<Arc<Framebuffer>>>,
}

impl Drm {
    pub fn new(device: Arc<dyn DrmDevice>) -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            sref: sref.clone(),

            inode: devfs::alloc_device_marker(),
            card_id: DRM_CARD_ID.fetch_add(1, Ordering::SeqCst),
            device,

            buffer_alloc: IdAllocator::new(),
            id_alloc: IdAllocator::new(),
            mapping_alloc: IdAllocator::new(),

            buffers: Mutex::new(HashMap::new()),
            mode_objs: Mutex::new(HashMap::new()),

            crtcs: Mutex::new(alloc::vec![]),
            encoders: Mutex::new(alloc::vec![]),
            connectors: Mutex::new(alloc::vec![]),
            framebuffers: Mutex::new(alloc::vec![]),
        })
    }

    /// Installs and initializes the CRTC identifier.
    pub fn install_crtc(&self, crtc: Arc<Crtc>) {
        self.crtcs.lock().push(crtc.clone());
        self.install_object(crtc)
    }

    /// Installs and initializes the connector identifier.
    pub fn install_connector(&self, connector: Arc<Connector>) {
        self.connectors.lock().push(connector.clone());
        self.install_object(connector)
    }

    /// Installs and initializes the encoder identifier.
    pub fn install_encoder(&self, encoder: Arc<Encoder>) {
        self.encoders.lock().push(encoder.clone());
        self.install_object(encoder)
    }

    /// Installs and initializes the framebuffer identifier.
    pub fn install_framebuffer(&self, fb: Arc<Framebuffer>) {
        self.framebuffers.lock().push(fb.clone());
        self.install_object(fb)
    }

    pub fn allocate_object_id(&self) -> u32 {
        self.id_alloc.alloc() as _
    }

    fn install_object(&self, object: Arc<dyn ModeObject>) {
        self.mode_objs.lock().insert(object.id(), object.clone());
    }

    fn find_object(&self, id: u32) -> Option<Arc<dyn ModeObject>> {
        self.mode_objs.lock().get(&id).cloned()
    }

    fn find_handle(&self, handle: u32) -> Option<BufferObject> {
        self.buffers.lock().get(&handle).cloned()
    }

    fn create_handle(&self, buffer: BufferObject) -> u32 {
        let handle = self.buffer_alloc.alloc() as u32;

        self.buffers.lock().insert(handle, buffer);
        handle
    }
}

impl INodeInterface for Drm {
    // The DRM is accessed using IOCTLs on a device representing a graphics
    // card.
    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            DRM_IOCTL_VERSION => {
                let mut struc = unsafe { UserRef::<DrmVersion>::new(VirtAddr::new(arg as u64)) };

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
                let mut struc = unsafe { UserRef::<DrmGetCap>::new(VirtAddr::new(arg as u64)) };

                // NOTE: The user is responsible for zeroing out the structure.
                match struc.capability {
                    DRM_CAP_DUMB_BUFFER => {
                        if self.device.can_dumb_create() {
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
                let mut struc =
                    unsafe { UserRef::<DrmModeCardRes>::new(VirtAddr::new(arg as u64)) };

                /// Copies the mode object IDs into the user provided buffer. For safety, checkout
                /// the [`copy_field`] function.
                fn copy_mode_obj_id<T: ModeObject>(
                    obj: &Mutex<Vec<Arc<T>>>,
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

            DRM_IOCTL_GET_CRTC => {
                let struc = unsafe { UserRef::<DrmModeCrtc>::new(VirtAddr::new(arg as u64)) };
                let _object = self.find_object(struc.crtc_id).unwrap().as_crtc().unwrap();

                log::warn!("drm::get_crtc: is a stub!");
                Ok(0)
            }

            DRM_IOCTL_SET_CRTC => {
                let struc = unsafe { UserRef::<DrmModeCrtc>::new(VirtAddr::new(arg as u64)) };
                let _object = self.find_object(struc.crtc_id).unwrap().as_crtc().unwrap();

                let object = self
                    .find_object(struc.fb_id)
                    .unwrap()
                    .as_framebuffer()
                    .unwrap();

                self.device.commit(&object.buffer_obj);
                log::warn!("drm::set_crtc: is a stub!");

                Ok(0)
            }

            DRM_IOCTL_GET_ENCODER => {
                let mut struc =
                    unsafe { UserRef::<DrmModeGetEncoder>::new(VirtAddr::new(arg as u64)) };

                let object = self
                    .find_object(struc.encoder_id)
                    .unwrap()
                    .as_encoder()
                    .unwrap();

                struc.crtc_id = object.current_crtc.id();

                let mut crtc_mask = 0;
                for crtc in object.possible_crtcs.iter() {
                    crtc_mask = 1 << crtc.index;
                }

                struc.possible_crtcs = crtc_mask;

                let mut clone_mask = 0;
                for clone in object.possible_clones.iter() {
                    clone_mask = 1 << clone.index;
                }

                struc.possible_clones = clone_mask;
                struc.encoder_typ = 0; // todo: fill in the encoder typ.

                Ok(0)
            }

            DRM_IOCTL_GET_CONNECTOR => {
                let mut struc =
                    unsafe { UserRef::<DrmModeGetConnector>::new(VirtAddr::new(arg as u64)) };

                let object = self
                    .find_object(struc.connector_id)
                    .unwrap()
                    .as_connector()
                    .unwrap();

                // Fill in the array containing all of the possible encoders and its length.
                let encoder_ids_ptr = struc.encoders_ptr as *mut u32;
                let mut encoder_count = 0;

                copy_field::<u32>(
                    encoder_ids_ptr,
                    &mut encoder_count,
                    object
                        .possible_encoders
                        .iter()
                        .map(|e| e.id())
                        .collect::<Vec<_>>()
                        .as_slice(),
                );

                struc.count_encoders = encoder_count as _;

                struc.encoder_id = object.current_encoder.id();
                struc.connector_type = object.connector_typ;
                struc.connector_type_id = 0; // todo
                struc.connection = object.status as _;

                // NOTE: The physical size will come from the EDID.
                struc.mm_width = 0; // todo
                struc.mm_height = 0; // todo
                struc.subpixel = 0; // todo

                // Fill in the array containing all of the possible modes and its length.
                let modes_ptr = struc.modes_ptr as *mut DrmModeInfo;
                let mut modes_count = 0;

                copy_field::<DrmModeInfo>(modes_ptr, &mut modes_count, object.modes.as_slice());
                struc.count_modes = modes_count as _;

                Ok(0)
            }

            DRM_IOCTL_MODE_CREATE_DUMB => {
                let mut struc =
                    unsafe { UserRef::<DrmModeCreateDumb>::new(VirtAddr::new(arg as u64)) };

                let (mut buffer, pitch) =
                    self.device
                        .dumb_create(struc.width, struc.height, struc.bpp);

                assert!(buffer.size < (1usize << 32));

                buffer.mapping = self.mapping_alloc.alloc() << 32;

                struc.pitch = pitch;
                struc.size = buffer.size as _;
                struc.handle = self.create_handle(buffer);

                Ok(0)
            }

            DRM_IOCTL_MODE_ADDFB => {
                let mut struc = unsafe { UserRef::<DrmModeFbCmd>::new(VirtAddr::new(arg as u64)) };

                let handle = self.find_handle(struc.handle).unwrap();
                self.device
                    .framebuffer_create(&handle, struc.width, struc.height, struc.pitch);

                let fb = Framebuffer::new(self.allocate_object_id(), handle);
                self.install_framebuffer(fb.clone());

                struc.fb_id = fb.id();
                Ok(0)
            }

            DRM_IOCTL_MODE_MAP_DUMB => {
                let mut struc =
                    unsafe { UserRef::<DrmModeMapDumb>::new(VirtAddr::new(arg as u64)) };

                let handle = self.find_handle(struc.handle).unwrap();
                struc.offset = handle.mapping as _;
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

    fn mmap(
        &self,
        offset: usize,
        _size: usize,
        _flags: aero_syscall::MMapFlags,
    ) -> fs::Result<PhysFrame> {
        let buffers = self.buffers.lock();
        let (_, handle) = buffers
            .iter()
            .find(|(_, h)| offset <= h.mapping + h.size && offset >= h.mapping)
            .unwrap();

        let index = (offset - handle.mapping) / Size4KiB::SIZE as usize;
        Ok(handle.memory[index])
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

fn make_mode_info(
    name: &str,
    typ: u32,
    clock: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    hskew: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    vscan: u16,
    flags: u32,
) -> DrmModeInfo {
    let mut this = DrmModeInfo {
        clock,
        hdisplay,
        hsync_start,
        hsync_end,
        htotal,
        hskew,
        vdisplay,
        vsync_start,
        vsync_end,
        vtotal,
        vscan,
        vrefresh: 0,
        flags,
        typ,
        name: [0; DRM_DISPLAY_MODE_LEN],
    };

    for (i, byte) in name.as_bytes().iter().enumerate() {
        this.name[i] = *byte as i8;
    }

    this
}

fn make_dmt_modes(max_width: u16, max_height: u16) -> Vec<DrmModeInfo> {
    #[rustfmt::skip] // with formatting this gets way too long.
    let modes = &[
        /* 0x01 - 640x350@85Hz */
        make_mode_info("640x350", DRM_MODE_TYPE_DRIVER, 31500, 640, 672,
        736, 832, 0, 350, 382, 385, 445, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x02 - 640x400@85Hz */
        make_mode_info("640x400", DRM_MODE_TYPE_DRIVER, 31500, 640, 672,
        736, 832, 0, 400, 401, 404, 445, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x03 - 720x400@85Hz */
        make_mode_info("720x400", DRM_MODE_TYPE_DRIVER, 35500, 720, 756,
        828, 936, 0, 400, 401, 404, 446, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x04 - 640x480@60Hz */
        make_mode_info("640x480", DRM_MODE_TYPE_DRIVER, 25175, 640, 656,
        752, 800, 0, 480, 490, 492, 525, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x05 - 640x480@72Hz */
        make_mode_info("640x480", DRM_MODE_TYPE_DRIVER, 31500, 640, 664,
        704, 832, 0, 480, 489, 492, 520, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x06 - 640x480@75Hz */
        make_mode_info("640x480", DRM_MODE_TYPE_DRIVER, 31500, 640, 656,
        720, 840, 0, 480, 481, 484, 500, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x07 - 640x480@85Hz */
        make_mode_info("640x480", DRM_MODE_TYPE_DRIVER, 36000, 640, 696,
        752, 832, 0, 480, 481, 484, 509, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x08 - 800x600@56Hz */
        make_mode_info("800x600", DRM_MODE_TYPE_DRIVER, 36000, 800, 824,
        896, 1024, 0, 600, 601, 603, 625, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x09 - 800x600@60Hz */
        make_mode_info("800x600", DRM_MODE_TYPE_DRIVER, 40000, 800, 840,
        968, 1056, 0, 600, 601, 605, 628, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x0a - 800x600@72Hz */
        make_mode_info("800x600", DRM_MODE_TYPE_DRIVER, 50000, 800, 856,
        976, 1040, 0, 600, 637, 643, 666, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x0b - 800x600@75Hz */
        make_mode_info("800x600", DRM_MODE_TYPE_DRIVER, 49500, 800, 816,
        896, 1056, 0, 600, 601, 604, 625, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x0c - 800x600@85Hz */
        make_mode_info("800x600", DRM_MODE_TYPE_DRIVER, 56250, 800, 832,
        896, 1048, 0, 600, 601, 604, 631, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x0d - 800x600@120Hz RB */
        make_mode_info("800x600", DRM_MODE_TYPE_DRIVER, 73250, 800, 848,
        880, 960, 0, 600, 603, 607, 636, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x0e - 848x480@60Hz */
        make_mode_info("848x480", DRM_MODE_TYPE_DRIVER, 33750, 848, 864,
        976, 1088, 0, 480, 486, 494, 517, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x0f - 1024x768@43Hz, interlace */
        make_mode_info("1024x768i", DRM_MODE_TYPE_DRIVER, 44900, 1024, 1032,
        1208, 1264, 0, 768, 768, 776, 817, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC |
        DRM_MODE_FLAG_INTERLACE),
        /* 0x10 - 1024x768@60Hz */
        make_mode_info("1024x768", DRM_MODE_TYPE_DRIVER, 65000, 1024, 1048,
        1184, 1344, 0, 768, 771, 777, 806, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x11 - 1024x768@70Hz */
        make_mode_info("1024x768", DRM_MODE_TYPE_DRIVER, 75000, 1024, 1048,
        1184, 1328, 0, 768, 771, 777, 806, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x12 - 1024x768@75Hz */
        make_mode_info("1024x768", DRM_MODE_TYPE_DRIVER, 78750, 1024, 1040,
        1136, 1312, 0, 768, 769, 772, 800, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x13 - 1024x768@85Hz */
        make_mode_info("1024x768", DRM_MODE_TYPE_DRIVER, 94500, 1024, 1072,
        1168, 1376, 0, 768, 769, 772, 808, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x14 - 1024x768@120Hz RB */
        make_mode_info("1024x768", DRM_MODE_TYPE_DRIVER, 115500, 1024, 1072,
        1104, 1184, 0, 768, 771, 775, 813, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x15 - 1152x864@75Hz */
        make_mode_info("1152x864", DRM_MODE_TYPE_DRIVER, 108000, 1152, 1216,
        1344, 1600, 0, 864, 865, 868, 900, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x55 - 1280x720@60Hz */
        make_mode_info("1280x720", DRM_MODE_TYPE_DRIVER, 74250, 1280, 1390,
        1430, 1650, 0, 720, 725, 730, 750, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x16 - 1280x768@60Hz RB */
        make_mode_info("1280x768", DRM_MODE_TYPE_DRIVER, 68250, 1280, 1328,
        1360, 1440, 0, 768, 771, 778, 790, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x17 - 1280x768@60Hz */
        make_mode_info("1280x768", DRM_MODE_TYPE_DRIVER, 79500, 1280, 1344,
        1472, 1664, 0, 768, 771, 778, 798, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x18 - 1280x768@75Hz */
        make_mode_info("1280x768", DRM_MODE_TYPE_DRIVER, 102250, 1280, 1360,
        1488, 1696, 0, 768, 771, 778, 805, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x19 - 1280x768@85Hz */
        make_mode_info("1280x768", DRM_MODE_TYPE_DRIVER, 117500, 1280, 1360,
        1496, 1712, 0, 768, 771, 778, 809, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x1a - 1280x768@120Hz RB */
        make_mode_info("1280x768", DRM_MODE_TYPE_DRIVER, 140250, 1280, 1328,
        1360, 1440, 0, 768, 771, 778, 813, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x1b - 1280x800@60Hz RB */
        make_mode_info("1280x800", DRM_MODE_TYPE_DRIVER, 71000, 1280, 1328,
        1360, 1440, 0, 800, 803, 809, 823, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x1c - 1280x800@60Hz */
        make_mode_info("1280x800", DRM_MODE_TYPE_DRIVER, 83500, 1280, 1352,
        1480, 1680, 0, 800, 803, 809, 831, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x1d - 1280x800@75Hz */
        make_mode_info("1280x800", DRM_MODE_TYPE_DRIVER, 106500, 1280, 1360,
        1488, 1696, 0, 800, 803, 809, 838, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x1e - 1280x800@85Hz */
        make_mode_info("1280x800", DRM_MODE_TYPE_DRIVER, 122500, 1280, 1360,
        1496, 1712, 0, 800, 803, 809, 843, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x1f - 1280x800@120Hz RB */
        make_mode_info("1280x800", DRM_MODE_TYPE_DRIVER, 146250, 1280, 1328,
        1360, 1440, 0, 800, 803, 809, 847, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x20 - 1280x960@60Hz */
        make_mode_info("1280x960", DRM_MODE_TYPE_DRIVER, 108000, 1280, 1376,
        1488, 1800, 0, 960, 961, 964, 1000, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x21 - 1280x960@85Hz */
        make_mode_info("1280x960", DRM_MODE_TYPE_DRIVER, 148500, 1280, 1344,
        1504, 1728, 0, 960, 961, 964, 1011, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x22 - 1280x960@120Hz RB */
        make_mode_info("1280x960", DRM_MODE_TYPE_DRIVER, 175500, 1280, 1328,
        1360, 1440, 0, 960, 963, 967, 1017, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x23 - 1280x1024@60Hz */
        make_mode_info("1280x1024", DRM_MODE_TYPE_DRIVER, 108000, 1280, 1328,
        1440, 1688, 0, 1024, 1025, 1028, 1066, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x24 - 1280x1024@75Hz */
        make_mode_info("1280x1024", DRM_MODE_TYPE_DRIVER, 135000, 1280, 1296,
        1440, 1688, 0, 1024, 1025, 1028, 1066, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x25 - 1280x1024@85Hz */
        make_mode_info("1280x1024", DRM_MODE_TYPE_DRIVER, 157500, 1280, 1344,
        1504, 1728, 0, 1024, 1025, 1028, 1072, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x26 - 1280x1024@120Hz RB */
        make_mode_info("1280x1024", DRM_MODE_TYPE_DRIVER, 187250, 1280, 1328,
        1360, 1440, 0, 1024, 1027, 1034, 1084, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x27 - 1360x768@60Hz */
        make_mode_info("1360x768", DRM_MODE_TYPE_DRIVER, 85500, 1360, 1424,
        1536, 1792, 0, 768, 771, 777, 795, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x28 - 1360x768@120Hz RB */
        make_mode_info("1360x768", DRM_MODE_TYPE_DRIVER, 148250, 1360, 1408,
        1440, 1520, 0, 768, 771, 776, 813, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x51 - 1366x768@60Hz */
        make_mode_info("1366x768", DRM_MODE_TYPE_DRIVER, 85500, 1366, 1436,
        1579, 1792, 0, 768, 771, 774, 798, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x56 - 1366x768@60Hz */
        make_mode_info("1366x768", DRM_MODE_TYPE_DRIVER, 72000, 1366, 1380,
        1436, 1500, 0, 768, 769, 772, 800, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x29 - 1400x1050@60Hz RB */
        make_mode_info("1400x1050", DRM_MODE_TYPE_DRIVER, 101000, 1400, 1448,
        1480, 1560, 0, 1050, 1053, 1057, 1080, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x2a - 1400x1050@60Hz */
        make_mode_info("1400x1050", DRM_MODE_TYPE_DRIVER, 121750, 1400, 1488,
        1632, 1864, 0, 1050, 1053, 1057, 1089, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x2b - 1400x1050@75Hz */
        make_mode_info("1400x1050", DRM_MODE_TYPE_DRIVER, 156000, 1400, 1504,
        1648, 1896, 0, 1050, 1053, 1057, 1099, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x2c - 1400x1050@85Hz */
        make_mode_info("1400x1050", DRM_MODE_TYPE_DRIVER, 179500, 1400, 1504,
        1656, 1912, 0, 1050, 1053, 1057, 1105, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x2d - 1400x1050@120Hz RB */
        make_mode_info("1400x1050", DRM_MODE_TYPE_DRIVER, 208000, 1400, 1448,
        1480, 1560, 0, 1050, 1053, 1057, 1112, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x2e - 1440x900@60Hz RB */
        make_mode_info("1440x900", DRM_MODE_TYPE_DRIVER, 88750, 1440, 1488,
        1520, 1600, 0, 900, 903, 909, 926, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x2f - 1440x900@60Hz */
        make_mode_info("1440x900", DRM_MODE_TYPE_DRIVER, 106500, 1440, 1520,
        1672, 1904, 0, 900, 903, 909, 934, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x30 - 1440x900@75Hz */
        make_mode_info("1440x900", DRM_MODE_TYPE_DRIVER, 136750, 1440, 1536,
        1688, 1936, 0, 900, 903, 909, 942, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x31 - 1440x900@85Hz */
        make_mode_info("1440x900", DRM_MODE_TYPE_DRIVER, 157000, 1440, 1544,
        1696, 1952, 0, 900, 903, 909, 948, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x32 - 1440x900@120Hz RB */
        make_mode_info("1440x900", DRM_MODE_TYPE_DRIVER, 182750, 1440, 1488,
        1520, 1600, 0, 900, 903, 909, 953, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x53 - 1600x900@60Hz */
        make_mode_info("1600x900", DRM_MODE_TYPE_DRIVER, 108000, 1600, 1624,
        1704, 1800, 0, 900, 901, 904, 1000, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x33 - 1600x1200@60Hz */
        make_mode_info("1600x1200", DRM_MODE_TYPE_DRIVER, 162000, 1600, 1664,
        1856, 2160, 0, 1200, 1201, 1204, 1250, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x34 - 1600x1200@65Hz */
        make_mode_info("1600x1200", DRM_MODE_TYPE_DRIVER, 175500, 1600, 1664,
        1856, 2160, 0, 1200, 1201, 1204, 1250, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x35 - 1600x1200@70Hz */
        make_mode_info("1600x1200", DRM_MODE_TYPE_DRIVER, 189000, 1600, 1664,
        1856, 2160, 0, 1200, 1201, 1204, 1250, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x36 - 1600x1200@75Hz */
        make_mode_info("1600x1200", DRM_MODE_TYPE_DRIVER, 202500, 1600, 1664,
        1856, 2160, 0, 1200, 1201, 1204, 1250, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x37 - 1600x1200@85Hz */
        make_mode_info("1600x1200", DRM_MODE_TYPE_DRIVER, 229500, 1600, 1664,
        1856, 2160, 0, 1200, 1201, 1204, 1250, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x38 - 1600x1200@120Hz RB */
        make_mode_info("1600x1200", DRM_MODE_TYPE_DRIVER, 268250, 1600, 1648,
        1680, 1760, 0, 1200, 1203, 1207, 1271, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x39 - 1680x1050@60Hz RB */
        make_mode_info("1680x1050", DRM_MODE_TYPE_DRIVER, 119000, 1680, 1728,
        1760, 1840, 0, 1050, 1053, 1059, 1080, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x3a - 1680x1050@60Hz */
        make_mode_info("1680x1050", DRM_MODE_TYPE_DRIVER, 146250, 1680, 1784,
        1960, 2240, 0, 1050, 1053, 1059, 1089, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x3b - 1680x1050@75Hz */
        make_mode_info("1680x1050", DRM_MODE_TYPE_DRIVER, 187000, 1680, 1800,
        1976, 2272, 0, 1050, 1053, 1059, 1099, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x3c - 1680x1050@85Hz */
        make_mode_info("1680x1050", DRM_MODE_TYPE_DRIVER, 214750, 1680, 1808,
        1984, 2288, 0, 1050, 1053, 1059, 1105, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x3d - 1680x1050@120Hz RB */
        make_mode_info("1680x1050", DRM_MODE_TYPE_DRIVER, 245500, 1680, 1728,
        1760, 1840, 0, 1050, 1053, 1059, 1112, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x3e - 1792x1344@60Hz */
        make_mode_info("1792x1344", DRM_MODE_TYPE_DRIVER, 204750, 1792, 1920,
        2120, 2448, 0, 1344, 1345, 1348, 1394, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x3f - 1792x1344@75Hz */
        make_mode_info("1792x1344", DRM_MODE_TYPE_DRIVER, 261000, 1792, 1888,
        2104, 2456, 0, 1344, 1345, 1348, 1417, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x40 - 1792x1344@120Hz RB */
        make_mode_info("1792x1344", DRM_MODE_TYPE_DRIVER, 333250, 1792, 1840,
        1872, 1952, 0, 1344, 1347, 1351, 1423, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x41 - 1856x1392@60Hz */
        make_mode_info("1856x1392", DRM_MODE_TYPE_DRIVER, 218250, 1856, 1952,
        2176, 2528, 0, 1392, 1393, 1396, 1439, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x42 - 1856x1392@75Hz */
        make_mode_info("1856x1392", DRM_MODE_TYPE_DRIVER, 288000, 1856, 1984,
        2208, 2560, 0, 1392, 1393, 1396, 1500, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x43 - 1856x1392@120Hz RB */
        make_mode_info("1856x1392", DRM_MODE_TYPE_DRIVER, 356500, 1856, 1904,
        1936, 2016, 0, 1392, 1395, 1399, 1474, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x52 - 1920x1080@60Hz */
        make_mode_info("1920x1080", DRM_MODE_TYPE_DRIVER, 148500, 1920, 2008,
        2052, 2200, 0, 1080, 1084, 1089, 1125, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x44 - 1920x1200@60Hz RB */
        make_mode_info("1920x1200", DRM_MODE_TYPE_DRIVER, 154000, 1920, 1968,
        2000, 2080, 0, 1200, 1203, 1209, 1235, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x45 - 1920x1200@60Hz */
        make_mode_info("1920x1200", DRM_MODE_TYPE_DRIVER, 193250, 1920, 2056,
        2256, 2592, 0, 1200, 1203, 1209, 1245, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x46 - 1920x1200@75Hz */
        make_mode_info("1920x1200", DRM_MODE_TYPE_DRIVER, 245250, 1920, 2056,
        2264, 2608, 0, 1200, 1203, 1209, 1255, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x47 - 1920x1200@85Hz */
        make_mode_info("1920x1200", DRM_MODE_TYPE_DRIVER, 281250, 1920, 2064,
        2272, 2624, 0, 1200, 1203, 1209, 1262, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x48 - 1920x1200@120Hz RB */
        make_mode_info("1920x1200", DRM_MODE_TYPE_DRIVER, 317000, 1920, 1968,
        2000, 2080, 0, 1200, 1203, 1209, 1271, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x49 - 1920x1440@60Hz */
        make_mode_info("1920x1440", DRM_MODE_TYPE_DRIVER, 234000, 1920, 2048,
        2256, 2600, 0, 1440, 1441, 1444, 1500, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x4a - 1920x1440@75Hz */
        make_mode_info("1920x1440", DRM_MODE_TYPE_DRIVER, 297000, 1920, 2064,
        2288, 2640, 0, 1440, 1441, 1444, 1500, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x4b - 1920x1440@120Hz RB */
        make_mode_info("1920x1440", DRM_MODE_TYPE_DRIVER, 380500, 1920, 1968,
        2000, 2080, 0, 1440, 1443, 1447, 1525, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x54 - 2048x1152@60Hz */
        make_mode_info("2048x1152", DRM_MODE_TYPE_DRIVER, 162000, 2048, 2074,
        2154, 2250, 0, 1152, 1153, 1156, 1200, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x4c - 2560x1600@60Hz RB */
        make_mode_info("2560x1600", DRM_MODE_TYPE_DRIVER, 268500, 2560, 2608,
        2640, 2720, 0, 1600, 1603, 1609, 1646, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x4d - 2560x1600@60Hz */
        make_mode_info("2560x1600", DRM_MODE_TYPE_DRIVER, 348500, 2560, 2752,
        3032, 3504, 0, 1600, 1603, 1609, 1658, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x4e - 2560x1600@75Hz */
        make_mode_info("2560x1600", DRM_MODE_TYPE_DRIVER, 443250, 2560, 2768,
        3048, 3536, 0, 1600, 1603, 1609, 1672, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x4f - 2560x1600@85Hz */
        make_mode_info("2560x1600", DRM_MODE_TYPE_DRIVER, 505250, 2560, 2768,
        3048, 3536, 0, 1600, 1603, 1609, 1682, 0,
        DRM_MODE_FLAG_NHSYNC | DRM_MODE_FLAG_PVSYNC),
        /* 0x50 - 2560x1600@120Hz RB */
        make_mode_info("2560x1600", DRM_MODE_TYPE_DRIVER, 552750, 2560, 2608,
        2640, 2720, 0, 1600, 1603, 1609, 1694, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x57 - 4096x2160@60Hz RB */
        make_mode_info("4096x2160", DRM_MODE_TYPE_DRIVER, 556744, 4096, 4104,
        4136, 4176, 0, 2160, 2208, 2216, 2222, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC),
        /* 0x58 - 4096x2160@59.94Hz RB */
        make_mode_info("4096x2160", DRM_MODE_TYPE_DRIVER, 556188, 4096, 4104,
        4136, 4176, 0, 2160, 2208, 2216, 2222, 0,
        DRM_MODE_FLAG_PHSYNC | DRM_MODE_FLAG_NVSYNC)
    ];

    let mut result = modes
        .iter()
        .filter(|e| e.hdisplay <= max_width && e.vdisplay <= max_height)
        .cloned()
        .collect::<Vec<_>>();

    // Sort the modes by display size:
    result.sort_by(|e, f| {
        ((e.hdisplay as usize) * (e.vdisplay as usize))
            .partial_cmp(&((f.hdisplay as usize) * (f.vdisplay as usize)))
            .unwrap() // unreachable
    });

    result
}
