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

// Address Resolution Protocol

use alloc::collections::BTreeMap;
use spin::{Once, RwLock};

use super::ethernet::MacAddr;
use super::ip::Ipv4Addr;

struct Entry {
    mac: MacAddr,
}

impl Entry {
    fn new(mac: MacAddr) -> Self {
        Self { mac }
    }
}

struct Cache(BTreeMap<Ipv4Addr, Entry>);

impl Cache {
    fn new() -> Self {
        Self(BTreeMap::new())
    }

    fn insert(&mut self, ip: Ipv4Addr, mac: MacAddr) {
        if let Some(_entry) = self.0.get_mut(&ip) {
            todo!()
        } else {
            self.0.insert(ip, Entry::new(mac));
        }
    }

    fn get(&self, ip: Ipv4Addr) -> Option<MacAddr> {
        if let Some(entry) = self.0.get(&ip) {
            return Some(entry.mac);
        }

        None
    }
}

static CACHE: Once<RwLock<Cache>> = Once::new();

pub fn get(ip: Ipv4Addr) -> Option<MacAddr> {
    CACHE
        .get()
        .as_ref()
        .expect("arp: cache not initialized")
        .read()
        .get(ip)
}

pub fn init() {
    CACHE.call_once(|| {
        let mut cache = Cache::new();
        cache.insert(Ipv4Addr::BROADCAST, MacAddr::BROADCAST);

        RwLock::new(cache)
    });
}
