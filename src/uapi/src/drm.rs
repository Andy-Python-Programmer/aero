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

// DRM IOCTL constants:
pub const DRM_IOCTL_VERSION: usize = drm_iowr::<DrmVersion>(0x00);
