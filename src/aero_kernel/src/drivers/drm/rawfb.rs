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

use crate::mem::paging::*;

use super::*;
use crate::rendy;

struct RawFramebuffer {}

impl DrmDevice for RawFramebuffer {
    fn can_dumb_create(&self) -> bool {
        true
    }

    fn dumb_create(&self, width: u32, height: u32, bpp: u32) -> (BufferObject, u32) {
        let size = align_up((width * height * bpp / 8) as _, Size4KiB::SIZE);
        let mut memory = alloc::vec![];

        for _ in (0..size).step_by(Size4KiB::SIZE as usize) {
            let frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR.allocate_frame().unwrap();
            memory.push(frame);
        }

        (BufferObject::new(size as usize, memory), width * bpp / 8)
    }

    fn commit(&self, buffer_obj: &BufferObject) {
        crate::rendy::DEBUG_RENDY
            .get()
            .map(|e| {
                let mut lock = e.lock_irq();
                let fb = lock.get_framebuffer();

                for (i, frame) in buffer_obj.memory.iter().enumerate() {
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            frame.as_slice_mut::<u8>().as_mut_ptr(),
                            (fb.as_mut_ptr() as *mut u8)
                                .offset(i as isize * Size4KiB::SIZE as isize),
                            4096,
                        )
                    }
                }
            })
            .unwrap();
    }

    fn framebuffer_create(
        &self,
        buffer_object: &BufferObject,
        _width: u32,
        height: u32,
        pitch: u32,
    ) {
        assert!(pitch % 4 == 0);
        assert!(buffer_object.size >= pitch as usize * height as usize);
    }

    fn driver_version(&self) -> (usize, usize, usize) {
        (0, 0, 1)
    }

    fn driver_info(&self) -> (&'static str, &'static str, &'static str) {
        ("rawfb_gpu", "rawfb gpu", "0")
    }

    fn min_dim(&self) -> (usize, usize) {
        // NOTE: for rawfb drm device, the max and min dimensions are the same.
        self.max_dim()
    }

    fn max_dim(&self) -> (usize, usize) {
        let info = rendy::get_rendy_info();
        (info.horizontal_resolution, info.vertical_resolution)
    }
}

fn init() {
    let info = rendy::get_rendy_info();

    let rfb = Drm::new(Arc::new(RawFramebuffer {}));
    let crtc = Crtc::new(&rfb, rfb.allocate_object_id());

    let encoder = Encoder::new(
        &rfb,
        crtc.clone(),
        alloc::vec![crtc.clone()],
        rfb.allocate_object_id(),
    );

    let connector = Connector::new(
        encoder.clone(),
        alloc::vec![encoder.clone()],
        make_dmt_modes(
            info.horizontal_resolution as u16,
            info.vertical_resolution as u16,
        ),
        DrmModeConStatus::Connected,
        rfb.allocate_object_id(),
    );

    let dri = devfs::DEV_FILESYSTEM
        .root_dir()
        .inode()
        .mkdir("dri")
        .expect("devfs: failed to create DRM directory");

    rfb.install_crtc(crtc);
    rfb.install_connector(connector);
    rfb.install_encoder(encoder);

    devfs::install_device_at(dri, rfb).expect("ramfs: failed to install DRM device");
}

crate::module_init!(init, ModuleType::Block);
