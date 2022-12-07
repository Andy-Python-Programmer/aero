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
use alloc::sync::Arc;

use spin::{Once, RwLock};

use crate::fs::lookup_path;
use crate::fs::Path;
use crate::logger;
use crate::mem::paging::*;
use crate::rendy::RendyInfo;

use super::cache::{DirCacheItem, INodeCacheItem};
use super::inode::{INodeInterface, PollFlags, PollTable};
use super::ramfs::RamFs;
use super::FileSystemError;
use super::{FileSystem, Result, MOUNT_MANAGER};

use aero_syscall::{prelude::*, MMapFlags};

lazy_static::lazy_static! {
    pub static ref DEV_FILESYSTEM: Arc<DevFs> = DevFs::new();
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
    install_device_at(DEV_FILESYSTEM.root_dir().inode(), device)
}

pub fn install_device_at(at: INodeCacheItem, device: Arc<dyn Device>) -> Result<()> {
    let devices = DEVICES.read();

    let device_marker = device.device_marker();
    let device_name = device.device_name();

    // We cannot have two devices with the same device marker.
    if devices.contains_key(&device_marker) {
        return Err(FileSystemError::EntryExists);
    }

    mem::drop(devices);

    DEVICES.write().insert(device_marker, device.clone());

    at.make_dev_inode(&device_name, device_marker)?;
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

    fn mmap(&self, offset: usize, size: usize, flags: MMapFlags) -> Result<PhysFrame> {
        self.0.inode().mmap(offset, size, flags)
    }

    fn ioctl(&self, command: usize, arg: usize) -> Result<usize> {
        self.0.inode().ioctl(command, arg)
    }

    fn poll(&self, table: Option<&mut PollTable>) -> Result<PollFlags> {
        self.0.inode().poll(table)
    }

    fn open(
        &self,
        flags: aero_syscall::OpenFlags,
        handle: Arc<super::file_table::FileHandle>,
    ) -> Result<Option<DirCacheItem>> {
        self.0.inode().open(flags, handle)
    }
}

/// Implementation of dev filesystem. (See the module-level documentation for more
/// information).
pub struct DevFs(Arc<RamFs>);

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

struct DevFb {
    marker: usize,
    vinfo: RwLock<FramebufferVScreenInfo>,
    finfo: FramebufferFScreenInfo,
}

impl DevFb {
    fn new(info: RendyInfo) -> Arc<Self> {
        Arc::new(Self {
            marker: alloc_device_marker(),
            vinfo: RwLock::new(FramebufferVScreenInfo {
                xres: info.horizontal_resolution as u32,
                yres: info.vertical_resolution as u32,

                xres_virtual: info.horizontal_resolution as u32,
                yres_virtual: info.vertical_resolution as u32,

                width: u32::MAX,  // -1
                height: u32::MAX, // -1

                red: FramebufferBitField::new(
                    info.red_mask_shift as u32,
                    info.red_mask_size as u32,
                ),

                green: FramebufferBitField::new(
                    info.green_mask_shift as u32,
                    info.green_mask_size as u32,
                ),

                blue: FramebufferBitField::new(
                    info.blue_mask_shift as u32,
                    info.blue_mask_size as u32,
                ),

                transp: FramebufferBitField::new(0, 0),
                bits_per_pixel: info.bits_per_pixel as u32,

                activate: FB_ACTIVATE_NOW,
                vmode: FB_VMODE_NONINTERLACED,

                // TODO: Implement rest of the members
                ..Default::default()
            }),

            finfo: FramebufferFScreenInfo {
                smem_len: (info.stride * info.vertical_resolution) as u32,
                line_length: info.stride as u32,

                typee: FB_TYPE_PACKED_PIXELS,
                visual: FB_VISUAL_TRUECOLOR,

                ..Default::default()
            },
        })
    }
}

impl Device for DevFb {
    fn device_marker(&self) -> usize {
        self.marker
    }

    fn device_name(&self) -> String {
        String::from("fb0")
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

    fn mmap(&self, offset: usize, size: usize, flags: MMapFlags) -> Result<PhysFrame> {
        let rinfo = crate::rendy::get_rendy_info();

        // Make sure we are in bounds.
        if offset > rinfo.byte_len || offset + Size4KiB::SIZE as usize > rinfo.byte_len {
            return Ok(PhysFrame::containing_address(PhysAddr::zero()));
        }

        crate::rendy::DEBUG_RENDY
            .get()
            .map(|e| unsafe {
                let mut lock = e.lock_irq();

                if flags.contains(MMapFlags::MAP_SHARED) {
                    // This is a shared file mapping.
                    let fb = lock.get_framebuffer();

                    let fb_ptr = fb.as_ptr() as *const u8;
                    let fb_ptr = fb_ptr.add(offset);

                    let fb_phys_ptr = fb_ptr.sub(crate::PHYSICAL_MEMORY_OFFSET.as_u64() as usize);

                    Ok(PhysFrame::containing_address(PhysAddr::new_unchecked(
                        fb_phys_ptr as u64,
                    )))
                } else {
                    let fb = lock.get_framebuffer();

                    // This is a private file mapping.
                    let private_cp: PhysFrame = FRAME_ALLOCATOR.allocate_frame().unwrap();
                    private_cp.as_slice_mut()[..size].copy_from_slice(&fb[offset..offset + size]);

                    Ok(private_cp)
                }
            })
            .expect("/dev/fb: terminal not initialized")
    }

    fn ioctl(&self, command: usize, arg: usize) -> Result<usize> {
        match command {
            FBIOGET_VSCREENINFO => {
                let struc = unsafe { &mut *(arg as *mut FramebufferVScreenInfo) };

                *struc = self.vinfo.read().clone();
                Ok(0x00)
            }

            FBIOPUT_VSCREENINFO => {
                let struc = unsafe { &mut *(arg as *mut FramebufferVScreenInfo) };
                *self.vinfo.write() = struc.clone();

                Ok(0x00)
            }

            FBIOGET_FSCREENINFO => {
                let struc = unsafe { &mut *(arg as *mut FramebufferFScreenInfo) };

                *struc = self.finfo.clone();
                Ok(0x00)
            }

            // Device independent colormap information can be get and set using
            // the `FBIOGETCMAP` and `FBIOPUTCMAP` ioctls.
            FBIOPUTCMAP => {
                let struc = VirtAddr::new(arg as _)
                    .read_mut::<FramebufferCmap>()
                    .ok_or(FileSystemError::NotSupported);

                log::debug!("fbdev: `FBIOPUTCMAP` is a stub! {struc:?}");
                Ok(0)
            }

            FBIOGETCMAP => {
                log::warn!("fbdev: `FBIOGETCMAP` is a stub!");
                Ok(0)
            }

            _ => {
                log::warn!("fbdev: ioctl unknown command: {command:#x}");
                Err(FileSystemError::NotSupported)
            }
        }
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

    let rendy_info = crate::rendy::get_rendy_info();

    {
        let null = DEV_NULL.call_once(|| DevNull::new());
        let kmsg = DEV_KMSG.call_once(|| DevKmsg::new());
        let fb = DEV_FB.call_once(|| DevFb::new(rendy_info));
        let urandom = DEV_URANDOM.call_once(|| DevUrandom::new());

        install_device(null.clone())?;
        install_device(kmsg.clone())?;
        install_device(fb.clone())?;
        install_device(urandom.clone())?;
    }

    Ok(())
}
