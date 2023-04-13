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

use bit_field::BitField;
use simple_endian::BigEndian;
use spin::RwLock;

use super::ip::Ipv4;
use super::{Packet, PacketHeader, PacketKind, PacketTrait, PacketUpHierarchy};

#[derive(Debug, Copy, Clone)]
pub struct Tcp {}

impl PacketKind for Tcp {}
impl PacketUpHierarchy<Tcp> for Packet<Ipv4> {}
impl PacketHeader<TcpHeader> for Packet<Tcp> {
    fn send(&self) {
        todo!()
    }

    fn recv(&self) {
        todo!()
    }
}

impl PacketTrait for Packet<Tcp> {
    fn header_size(&self) -> usize {
        self.header().header_len() as usize
    }
}

#[repr(C, packed)]
pub struct TcpHeader {
    src_port: BigEndian<u16>,
    dst_port: BigEndian<u16>,
    seq_nr: BigEndian<u32>,
    ack_nr: BigEndian<u32>,
    flags: BigEndian<u16>,
    window: BigEndian<u16>,
    checksum: BigEndian<u16>,
    urgent_ptr: BigEndian<u16>,
}

impl TcpHeader {
    /// Return the header length, in octets.
    pub fn header_len(&self) -> u8 {
        let flags = self.flags;
        (flags.to_native().get_bits(12..=15) * 4) as u8
    }
}

static HANDLERS: RwLock<BTreeMap<u16, Arc<dyn TcpHandler>>> = RwLock::new(BTreeMap::new());

pub trait TcpHandler: Send + Sync {
    fn recv(&self, packet: Packet<Tcp>);
}

pub fn alloc_ephemeral_port(socket: Arc<dyn TcpHandler>) -> Option<u16> {
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
