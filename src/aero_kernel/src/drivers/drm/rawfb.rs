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

use alloc::sync::Arc;

use crate::fs::devfs;
use crate::fs::FileSystem;

use super::{Drm, DrmDevice};

struct RawFramebuffer {}

impl DrmDevice for RawFramebuffer {
    fn dumb_create(&self) -> bool {
        true
    }

    fn driver_version(&self) -> (usize, usize, usize) {
        (0, 0, 1)
    }

    fn driver_info(&self) -> (&'static str, &'static str, &'static str) {
        ("rawfb_gpu", "rawfb gpu", "0")
    }

    fn min_dim(&self) -> (usize, usize) {
        todo!()
    }

    fn max_dim(&self) -> (usize, usize) {
        todo!()
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
