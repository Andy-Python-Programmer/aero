/*
 * Copyright (C) 2021-2023 The Aero Project Developers.
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

use alloc::{sync::Arc, vec::Vec};
use spin::RwLock;

pub mod arp;
pub mod dhcp;
pub mod ethernet;
pub mod ip;

pub use ethernet::MacAddr;

pub trait NetworkDevice: Send + Sync {
    fn send(&self, packet: ethernet::Packet);
    fn mac(&self) -> MacAddr;
}

static DEVICES: RwLock<Vec<Arc<dyn NetworkDevice>>> = RwLock::new(Vec::new());
static DEFAULT_DEVICE: RwLock<Option<Arc<dyn NetworkDevice>>> = RwLock::new(None);

pub fn add_device(device: Arc<dyn NetworkDevice>) {
    DEVICES.write().push(device.clone());

    let mut default_device = DEFAULT_DEVICE.write();
    if default_device.is_none() {
        *default_device = Some(device);
    }
}

pub fn default_device() -> Arc<dyn NetworkDevice> {
    DEFAULT_DEVICE
        .read()
        .as_ref()
        .expect("net: no devices found")
        .clone()
}

// Initialize the networking stack.
pub fn init() {
    if DEFAULT_DEVICE.read().is_none() {
        // No network devices are avaliable.
        return;
    }

    arp::init();
    dhcp::init();
}
