/*
 * Copyright 1999 Precision Insight, Inc., Cedar Park, Texas.
 * Copyright 2000 VA Linux Systems, Inc., Sunnyvale, California.
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * All rights reserved.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice (including the next
 * paragraph) shall be included in all copies or substantial portions of the
 * Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL
 * VA LINUX SYSTEMS AND/OR ITS SUPPLIERS BE LIABLE FOR ANY CLAIM, DAMAGES OR
 * OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
 * ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
 * OTHER DEALINGS IN THE SOFTWARE.
 */

use crate::ioctl;
use core::ffi;

pub const DRM_IOCTL_BASE: usize = 'd' as usize;

// Functions to generate the IOCTl numbers:
#[inline]
pub const fn drm_io(nr: usize) -> usize {
    ioctl::io(DRM_IOCTL_BASE, nr)
}

#[inline]
pub const fn drm_ior<T>(nr: usize) -> usize {
    ioctl::ior::<T>(DRM_IOCTL_BASE, nr)
}

#[inline]
pub const fn drm_iow<T>(nr: usize) -> usize {
    ioctl::iow::<T>(DRM_IOCTL_BASE, nr)
}

#[inline]
pub const fn drm_iowr<T>(nr: usize) -> usize {
    ioctl::iowr::<T>(DRM_IOCTL_BASE, nr)
}

// DRM structures:
#[repr(C)]
pub struct DrmVersion {
    pub version_major: ffi::c_int,
    pub version_minor: ffi::c_int,
    pub version_patch_level: ffi::c_int,

    pub name_len: usize,
    pub name: *mut u8, // name of the driver

    pub date_len: usize,
    pub date: *mut u8, // buffer to hold date

    pub desc_len: usize,
    pub desc: *mut u8, // buffer to hold desc
}

// Refer to the `libdrm` documentation for more information about the
// capabilities.
pub const DRM_CAP_DUMB_BUFFER: u64 = 0x01;
pub const DRM_CAP_VBLANK_HIGH_CRTC: u64 = 0x02;
pub const DRM_CAP_DUMB_PREFERRED_DEPTH: u64 = 0x03;
pub const DRM_CAP_DUMB_PREFER_SHADOW: u64 = 0x04;
pub const DRM_CAP_PRIME: u64 = 0x05;
pub const DRM_PRIME_CAP_IMPORT: u64 = 0x01;
pub const DRM_PRIME_CAP_EXPORT: u64 = 0x02;
pub const DRM_CAP_TIMESTAMP_MONOTONIC: u64 = 0x06;
pub const DRM_CAP_ASYNC_PAGE_FLIP: u64 = 0x07;
pub const DRM_CAP_CURSOR_WIDTH: u64 = 0x08;
pub const DRM_CAP_CURSOR_HEIGHT: u64 = 0x09;
pub const DRM_CAP_ADDFB2_MODIFIERS: u64 = 0x10;
pub const DRM_CAP_PAGE_FLIP_TARGET: u64 = 0x11;
pub const DRM_CAP_CRTC_IN_VBLANK_EVENT: u64 = 0x12;
pub const DRM_CAP_SYNCOBJ: u64 = 0x13;
pub const DRM_CAP_SYNCOBJ_TIMELINE: u64 = 0x14;

#[repr(C)]
pub struct DrmGetCap {
    pub capability: u64,
    pub value: u64,
}

#[repr(C)]
pub struct DrmModeCardRes {
    pub fb_id_ptr: u64,
    pub crtc_id_ptr: u64,
    pub connector_id_ptr: u64,
    pub encoder_id_ptr: u64,
    pub count_fbs: u32,
    pub count_crtcs: u32,
    pub count_connectors: u32,
    pub count_encoders: u32,
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum DrmModeConStatus {
    Connected = 1, // connector has the sink plugged in
    Disconnected = 2,
    Unknown = 3,
}

const DRM_DISPLAY_MODE_LEN: usize = 32;

#[repr(C)]
pub struct DrmModeInfo {
    pub clock: u32,                                // pixel clock in kHz
    pub hdisplay: u16,                             // horizontal display size
    pub hsync_start: u16,                          // horizontal sync start
    pub hsync_end: u16,                            // horizontal sync end
    pub htotal: u16,                               // horizontal total size
    pub hskew: u16,                                // horizontal skew
    pub vdisplay: u16,                             // vertical display size
    pub vsync_start: u16,                          // vertical sync start
    pub vsync_end: u16,                            // vertical sync end
    pub vtotal: u16,                               // vertical total size
    pub vscan: u16,                                // vertical scan
    pub vrefresh: u32,                             // approximate vertical refresh rate in Hz
    pub flags: u32,                                // bitmask of misc flags
    pub typ: u32,                                  // bitmask of type flags
    pub name: [ffi::c_char; DRM_DISPLAY_MODE_LEN], // string describing the mode resolution
}

#[repr(C)]
pub struct DrmModeGetConnector {
    pub encoders_ptr: u64,    // pointer to `u32` array of object IDs
    pub modes_ptr: u64,       // pointer to `DrmModeInfo` array
    pub props_ptr: u64,       // pointer to `u32` array of property IDs
    pub prop_values_ptr: u64, // pointer to `u64` array of property values

    pub count_modes: u32,    // number of modes
    pub count_props: u32,    // number of properties
    pub count_encoders: u32, // number of encoders

    pub encoder_id: u32,     // object id of the current encoder
    pub connector_id: u32,   // object id of the connector
    pub connector_type: u32, // type of the connector

    /// Type-specific connector number.
    ///
    /// This is not an object ID. This is a per-type connector number. Each
    /// (`type`, `type_id`) combination is unique across all connectors of a DRM
    /// device.
    pub connector_type_id: u32,

    pub connection: u32, // status of the connector
    pub mm_width: u32,   // width of the connected sink in millimeters
    pub mm_height: u32,  // height of the connected sink in millimeters

    pub subpixel: u32, // subpixel order of the connected sink

    pub pad: u32, // padding; must be zero
}

// DRM IOCTL constants:
pub const DRM_IOCTL_VERSION: usize = drm_iowr::<DrmVersion>(0x00);
pub const DRM_IOCTL_GET_CAP: usize = drm_iowr::<DrmGetCap>(0x0c);
pub const DRM_IOCTL_MODE_GETRESOURCES: usize = drm_iowr::<DrmModeCardRes>(0xa0);
pub const DRM_IOCTL_GET_CONNECTOR: usize = drm_iowr::<DrmModeGetConnector>(0xa7);
