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

use super::ip::{Ipv4, Ipv4Addr};
use super::{ip, PacketDownHierarchy};

use super::{ConstPacketKind, Packet, PacketHeader, PacketUpHierarchy};

#[derive(Copy, Clone)]
pub struct Udp;

impl ConstPacketKind for Udp {
    const HSIZE: usize = core::mem::size_of::<Header>();
}

impl Packet<Udp> {
    pub fn create(src_port: u16, dest_port: u16, mut size: usize, target: Ipv4Addr) -> Packet<Udp> {
        size += Udp::HSIZE;

        let ip_packet = Packet::<Ipv4>::create(ip::Type::Udp, target, size);
        let mut packet = ip_packet.upgrade();

        let header = packet.header_mut();

        header.src_port = BigEndian::from(src_port);
        header.dst_port = BigEndian::from(dest_port);
        header.len = BigEndian::from(size as u16);

        packet
    }
}

impl PacketUpHierarchy<Udp> for Packet<Ipv4> {}
impl PacketHeader<Header> for Packet<Udp> {
    fn send(&self) {
        {
            let mut this = self.clone();
            let header = this.header_mut();
            header.compute_checksum(self.downgrade().header());
        }

        self.downgrade().send() // send the IP packet
    }
}

#[repr(C, packed)]
pub struct Header {
    pub src_port: BigEndian<u16>,
    pub dst_port: BigEndian<u16>,
    pub len: BigEndian<u16>,
    pub crc: BigEndian<u16>,
}

impl Header {
    fn compute_checksum(&mut self, _header: &ip::Header) {
        self.crc = BigEndian::from(0);
        // FIXME:
    }
}
