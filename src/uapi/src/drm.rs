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

// DRM IOCTL constants:
pub const DRM_IOCTL_VERSION: usize = drm_iowr::<DrmVersion>(0x00);
pub const DRM_IOCTL_GET_CAP: usize = drm_iowr::<DrmGetCap>(0x0c);
