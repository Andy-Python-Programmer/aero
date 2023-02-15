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

use simple_endian::BigEndian;

use super::*;

/// Size of IPv4 adderess in octets.
///
/// [RFC 8200 § 2]: https://www.rfc-editor.org/rfc/rfc791#section-3.2
pub const ADDR_SIZE: usize = 4;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default)]
pub struct Ipv4Addr(pub [u8; ADDR_SIZE]);

impl Ipv4Addr {
    pub const BROADCAST: Self = Self([0xff; ADDR_SIZE]);
    pub const EMPTY: Self = Self([0x00; ADDR_SIZE]);
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Type {
    Udp = 17u8.swap_bytes(),
}

#[repr(C, packed)]
pub struct Header {
    pub v: BigEndian<u8>,
    pub tos: BigEndian<u8>,
    pub length: BigEndian<u16>,
    pub ident: BigEndian<u16>,
    pub frag_offset: BigEndian<u16>,
    pub ttl: BigEndian<u8>,
    pub protocol: Type,
    pub hcrc: BigEndian<u16>,
    pub src_ip: Ipv4Addr,
    pub dest_ip: Ipv4Addr,
}

impl Header {
    /// Set the payload length.
    fn set_length(&mut self, length: u16) {
        self.length = BigEndian::from(length);
    }
}

#[derive(Clone)]
pub struct Ipv4;

impl ConstPacketKind for Ipv4 {
    const HSIZE: usize = core::mem::size_of::<Header>();
}

impl Packet<Ipv4> {
    pub fn create(protocol: Type, dest: Ipv4Addr, mut size: usize) -> Packet<Ipv4> {
        size += Ipv4::HSIZE;

        let mut packet = Packet::<Eth>::create(ethernet::Type::Ip, size).upgrade();
        let header = packet.header_mut();

        header.v = BigEndian::<u8>::from(0x45);
        header.tos = BigEndian::<u8>::from(0);
        header.ident = BigEndian::<u16>::from(0);
        header.frag_offset = BigEndian::<u16>::from(0);
        header.ttl = BigEndian::<u8>::from(64);
        header.hcrc = BigEndian::<u16>::from(0);

        header.set_length(size as _);

        header.protocol = protocol;
        header.dest_ip = dest;

        // FIXME: Set the source IPv4 address.
        header.src_ip = Ipv4Addr([0; 4]);

        // TODO: Header checksum
        packet
    }
}

impl PacketUpHierarchy<Ipv4> for Packet<Eth> {}
impl PacketHeader<Header> for Packet<Ipv4> {
    fn send(&self) {
        {
            let mut this = self.clone();
            let header = this.header_mut();
            {
                header.hcrc = BigEndian::from(0);
                // FIXME:
            }
        }
        self.downgrade().send() // send the ethernet packet
    }
}
