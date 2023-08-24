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

//! Address Resolution Protocol

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use byte_endian::BigEndian;
use spin::{Once, RwLock};

use crate::net::{default_device, ethernet, PacketUpHierarchy};

use super::ethernet::MacAddr;
use super::ip::Ipv4Addr;
use super::{ConstPacketKind, Eth, Packet, PacketDownHierarchy, PacketHeader};

enum Status {
    Resolved,
    Pending(Vec<Packet<Eth>>),
}

struct Entry {
    mac: MacAddr,
    status: Status,
}

impl Entry {
    fn new(mac: MacAddr, status: Status) -> Self {
        Self { mac, status }
    }
}

struct Cache(BTreeMap<Ipv4Addr, Entry>);

impl Cache {
    fn new() -> Self {
        Self(BTreeMap::new())
    }

    fn insert(&mut self, ip: Ipv4Addr, mac: MacAddr) {
        if let Some(entry) = self.0.get_mut(&ip) {
            let status = core::mem::replace(&mut entry.status, Status::Resolved);

            if let Status::Pending(queue) = status {
                entry.mac = mac;
                entry.status = Status::Resolved;

                for mut packet in queue {
                    log::trace!("[ ARP ] (!!) Sending queued packed to {ip:?} {mac:?}");
                    packet.header_mut().dest_mac = mac;
                    super::default_device().send(packet);
                }
            }
        } else {
            self.0.insert(ip, Entry::new(mac, Status::Resolved));
        }
    }

    fn request(&mut self, ip: Ipv4Addr, packet: Packet<Eth>) {
        if self.0.get_mut(&ip).is_some() {
            todo!()
        } else {
            let queue = alloc::vec![packet];
            let entry = Entry::new(MacAddr::NULL, Status::Pending(queue));

            self.0.insert(ip, entry);
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

/// Hardware Address Space (e.g., Ethernet, Packet Radio Net.)
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum HType {
    Ethernet = 1u16.swap_bytes(),
}

/// Internetwork Protocol for which the ARP request is intended.
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum PType {
    Ipv4 = 0x0800u16.swap_bytes(),
}

/// ARP Opcode
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub enum Opcode {
    Request = 1u16.swap_bytes(),
    Reply = 2u16.swap_bytes(),
}

#[repr(C, packed)]
pub struct ArpHeader {
    pub htype: HType,
    pub ptype: PType,
    /// Length (in octets) of a hardware address.
    pub hlen: BigEndian<u8>,
    /// Length (in octets) of internetwork addresses.
    pub plen: BigEndian<u8>,
    pub opcode: Opcode,
    pub src_mac: MacAddr,
    pub src_ip: Ipv4Addr,
    pub dest_mac: MacAddr,
    pub dest_ip: Ipv4Addr,
}

impl ArpHeader {
    pub fn opcode(&self) -> Opcode {
        self.opcode
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Arp {}

impl Packet<Arp> {
    pub fn create() -> Packet<Arp> {
        let device = default_device();

        let mut packet: Packet<Arp> =
            Packet::<Eth>::create(ethernet::Type::Arp, core::mem::size_of::<ArpHeader>()).upgrade();

        let header = packet.header_mut();
        header.htype = HType::Ethernet;
        header.ptype = PType::Ipv4;
        header.hlen = BigEndian::from(MacAddr::ADDR_SIZE as u8);
        header.plen = BigEndian::from(Ipv4Addr::ADDR_SIZE as u8);

        header.src_ip = device.ip();
        header.src_mac = device.mac();

        packet
    }
}

impl ConstPacketKind for Arp {
    const HSIZE: usize = core::mem::size_of::<ArpHeader>();
}

impl PacketUpHierarchy<Arp> for Packet<Eth> {}
impl PacketHeader<ArpHeader> for Packet<Arp> {
    fn send(&self) {
        self.downgrade().send()
    }

    fn recv(&self) {
        let header = self.header();

        CACHE
            .get()
            .as_ref()
            .expect("arp: cache not initialized")
            .write()
            .insert(header.src_ip, header.src_mac);

        let device = default_device();

        if header.opcode() == Opcode::Request && header.dest_ip == device.ip() {
            let mut packet = Packet::<Arp>::create();
            let reply_header = packet.header_mut();

            reply_header.opcode = Opcode::Reply;
            reply_header.dest_ip = header.src_ip;
            reply_header.dest_mac = header.src_mac;

            packet.send();
        }
    }
}

pub fn request_ip(target: Ipv4Addr, to: Packet<Eth>) {
    let mut packet = Packet::<Arp>::create();
    let header = packet.header_mut();

    header.opcode = Opcode::Request;
    header.dest_ip = target;
    header.dest_mac = MacAddr::NULL;

    log::debug!("[ ARP ] (!!) Sending request for {target:?}");

    CACHE
        .get()
        .as_ref()
        .expect("arp: cache not initialized")
        .write()
        .request(target, to);

    packet.send();
}
