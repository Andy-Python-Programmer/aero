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

use super::ip::{self, Ipv4, Ipv4Addr};
use super::{
    checksum, Packet, PacketDownHierarchy, PacketHeader, PacketKind, PacketTrait, PacketUpHierarchy,
};

#[derive(Debug, Copy, Clone)]
pub struct Tcp {}

impl Packet<Tcp> {
    pub fn create(src_port: u16, dest_port: u16, size: usize, target: Ipv4Addr) -> Packet<Tcp> {
        let total_size = core::mem::size_of::<TcpHeader>() + size;
        let mut packet: Packet<Tcp> =
            Packet::<Ipv4>::create(ip::Type::Tcp, target, total_size).upgrade();

        let header = packet.header_mut();

        header.src_port = src_port.into();
        header.dest_port = dest_port.into();
        header.set_header_len(core::mem::size_of::<TcpHeader>() as u8);

        packet
    }

    pub fn ack_len(&self) -> u32 {
        let data_size = self.as_slice().len() as u32;
        let flags = self.header().flags();

        let mut addend = 0;
        if flags.contains(TcpFlags::FIN) | flags.contains(TcpFlags::SYN) {
            addend = 1;
        }

        data_size + addend
    }
}

bitflags::bitflags! {
    pub struct TcpFlags: u16 {
        const FIN = 1 << 0;
        const SYN = 1 << 1;
        const RST = 1 << 2;
        const PSH = 1 << 3;
        const ACK = 1 << 4;
        const URG = 1 << 5;
    }
}

impl PacketKind for Tcp {}
impl PacketUpHierarchy<Tcp> for Packet<Ipv4> {}
impl PacketHeader<TcpHeader> for Packet<Tcp> {
    fn send(&self) {
        let ip_packet = self.downgrade();

        let mut packet = *self;
        let header = packet.header_mut();

        header.compute_checksum(ip_packet.header());
        ip_packet.send();
    }

    fn recv(&self) {
        let header = self.header();
        let dest_port = header.dest_port();

        let handlers = HANDLERS.read();

        if let Some(handler) = handlers.get(&dest_port) {
            handler.recv(*self);
        } else {
            log::warn!("tcp: no handler registered for port {}", dest_port);
        }
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
    dest_port: BigEndian<u16>,
    seq_nr: BigEndian<u32>,
    ack_nr: BigEndian<u32>,
    flags: BigEndian<u16>,
    window: BigEndian<u16>,
    checksum: BigEndian<u16>,
    urgent_ptr: BigEndian<u16>,
}

const_assert_eq!(core::mem::size_of::<TcpHeader>(), 20);

impl TcpHeader {
    /// Return the header length, in octets.
    pub fn header_len(&self) -> u8 {
        let flags = self.flags;
        (flags.to_native().get_bits(12..=15) * 4) as u8
    }

    /// Sets the ACK number to `val` and sets the [`TcpFlags::ACK`] flag.
    pub fn set_ack_number(&mut self, val: u32) {
        self.ack_nr = val.into();

        let mut flags = self.flags();
        flags.insert(TcpFlags::ACK);
        self.set_flags(flags);
    }

    pub fn set_header_len(&mut self, val: u8) {
        let mut flags = self.flags.to_native();
        flags.set_bits(12..=15, val as u16 / 4);
        self.flags = flags.into();
    }

    pub fn compute_checksum(&mut self, ip_header: &ip::Header) {
        self.checksum = BigEndian::from(0);
        self.checksum = checksum::make_combine(&[
            checksum::calculate(&checksum::PseudoHeader::new(ip_header)),
            checksum::calculate_with_len(
                self,
                ip_header.length() as usize - core::mem::size_of::<ip::Header>(),
            ),
        ]);
    }

    pub fn dest_port(&self) -> u16 {
        self.dest_port.to_native()
    }

    pub fn set_sequence_number(&mut self, val: u32) {
        self.seq_nr = val.into();
    }

    pub fn sequence_number(&self) -> u32 {
        self.seq_nr.to_native()
    }

    pub fn set_window(&mut self, val: u16) {
        self.window = val.into();
    }

    pub fn set_flags(&mut self, val: TcpFlags) {
        let mut flags = self.flags.to_native();
        flags.set_bits(0..=5, val.bits());
        self.flags = flags.into();
    }

    pub fn flags(&self) -> TcpFlags {
        let raw = self.flags.to_native().get_bits(0..=5);
        TcpFlags::from_bits_truncate(raw)
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
