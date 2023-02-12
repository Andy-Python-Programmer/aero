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

use super::arp;
use super::ip::Ipv4Addr;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
#[repr(C)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    pub const BROADCAST: Self = Self([0xff; 6]);
}

#[repr(u16)]
pub enum Type {
    Ip = 0x800,
}

#[repr(C, packed)]
pub struct Packet {
    pub dest_mac: MacAddr,
    pub src_mac: MacAddr,
    pub typ: Type,
}

impl Packet {
    /// Creates a new ethernet packet.
    pub fn new(typ: Type) -> Self {
        let src_mac = super::default_device().mac();
        Self {
            src_mac,
            dest_mac: MacAddr([0; 6]),
            typ,
        }
    }
}

pub fn send_packet(mut packet: Packet, ip: Ipv4Addr) {
    if let Some(addr) = arp::get(ip) {
        packet.dest_mac = addr;
        super::default_device().send(packet);
    }
}
