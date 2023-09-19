// Copyright (C) 2021-2023 The Aero Project Developers.
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

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use spin::RwLock;

use netstack::network::Ipv4Addr;
use netstack::transport::Udp;

pub fn do_recv(udp: &Udp, payload: &[u8]) {
    let dest_port = udp.dst_port();

    let handlers = HANDLERS.read();

    if let Some(handler) = handlers.get(&dest_port) {
        handler.recv(udp, payload);
    } else {
        log::warn!("udp: no handler registered for port {}", dest_port);
    }
}

static HANDLERS: RwLock<BTreeMap<u16, Arc<dyn UdpHandler>>> = RwLock::new(BTreeMap::new());

pub trait UdpHandler: Send + Sync {
    fn recv(&self, udp: &Udp, payload: &[u8]);
}

pub fn alloc_ephemeral_port(socket: Arc<dyn UdpHandler>) -> Option<u16> {
    const EPHEMERAL_START: u16 = 49152;
    const EPHEMERAL_END: u16 = u16::MAX;

    let mut handlers = HANDLERS.write();

    // Ephemeral ports in the range 49152..65535 are not
    // assigned, controlled, or registered and are used
    // for temporary or private ports.
    for port in EPHEMERAL_START..=EPHEMERAL_END {
        if handlers.contains_key(&port) {
            continue;
        }

        handlers.insert(port, socket);
        return Some(port);
    }

    None
}

pub fn bind(port: u16, socket: Arc<dyn UdpHandler>) {
    log::trace!("udp: bind(port={port})");

    let mut handlers = HANDLERS.write();
    // check if the port is already in use
    assert!(!handlers.contains_key(&port));

    handlers.insert(port, socket);
}

pub fn connect(host: Ipv4Addr, port: u16) {
    log::trace!("udp: connect(host={host:?}, port={port})");
}
