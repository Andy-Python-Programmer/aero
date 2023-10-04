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
use spin::{Once, RwLock};

use crate::net::default_device;
use crate::net::shim::PacketSend;

use crabnet::data_link::{Arp, ArpAddress, ArpHardwareType, ArpOpcode, Eth, EthType, MacAddr};
use crabnet::network::Ipv4Addr;

use super::RawPacket;

enum Status {
    Resolved,
    Pending(Vec<RawPacket>),
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

                    // FIXME: make this cleaner
                    let eth = unsafe { &mut *packet.as_mut_ptr().cast::<Eth>() };
                    eth.dest_mac = mac;

                    super::default_device().send(packet);
                }
            }
        } else {
            self.0.insert(ip, Entry::new(mac, Status::Resolved));
        }
    }

    fn request(&mut self, ip: Ipv4Addr, packet: RawPacket) {
        if ip == Ipv4Addr::LOOPBACK {
            panic!()
        }

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

// #[derive(Debug, Copy, Clone)]
// pub struct Arp {}

// impl Packet<Arp> {
//     pub fn create() -> Packet<Arp> {
//         let device = default_device();

//         let mut packet: Packet<Arp> =
//             Packet::<Eth>::create(ethernet::Type::Arp,
// core::mem::size_of::<ArpHeader>()).upgrade();

//         let header = packet.header_mut();
//         header.htype = HType::Ethernet;
//         header.ptype = PType::Ipv4;
//         header.hlen = BigEndian::from(MacAddr::ADDR_SIZE as u8);
//         header.plen = BigEndian::from(Ipv4Addr::ADDR_SIZE as u8);

//         header.src_ip = device.ip();
//         header.src_mac = device.mac();

//         packet
//     }
// }

// impl ConstPacketKind for Arp {
//     const HSIZE: usize = core::mem::size_of::<ArpHeader>();
// }

// impl PacketUpHierarchy<Arp> for Packet<Eth> {}
// impl PacketHeader<ArpHeader> for Packet<Arp> {
//     fn recv(&self) {
//         let header = self.header();

//         CACHE
//             .get()
//             .as_ref()
//             .expect("arp: cache not initialized")
//             .write()
//             .insert(header.src_ip, header.src_mac);

//         let device = default_device();

//         if header.opcode() == Opcode::Request && header.dest_ip == device.ip() {
//             let mut packet = Packet::<Arp>::create();
//             let reply_header = packet.header_mut();

//             reply_header.opcode = Opcode::Reply;
//             reply_header.dest_ip = header.src_ip;
//             reply_header.dest_mac = header.src_mac;

//             packet.send();
//         }
//     }
// }

pub fn do_recv(arp: &Arp) {
    CACHE
        .get()
        .as_ref()
        .expect("arp: cache not initialized")
        .write()
        .insert(arp.src_ip(), arp.src_mac());

    let device = default_device();

    if arp.opcode() == ArpOpcode::Request && arp.dest_ip() == device.ip() {
        let addr = ArpAddress::new(arp.src_mac(), arp.src_ip());
        let reply_arp = make_arp(ArpOpcode::Reply, addr);

        reply_arp.send();
    }
}

pub fn request_ip(target: Ipv4Addr, to: RawPacket) {
    let arp = make_arp(ArpOpcode::Request, ArpAddress::new(MacAddr::NULL, target));

    log::debug!("[ ARP ] (!!) Sending request for {target:?}");

    CACHE
        .get()
        .as_ref()
        .expect("arp: cache not initialized")
        .write()
        .request(target, to);

    arp.send();
}

fn make_arp(opcode: ArpOpcode, dest_addr: ArpAddress) -> Arp {
    let device = default_device();
    let src_addr = ArpAddress::new(device.mac(), device.ip());

    Arp::new(
        ArpHardwareType::Ethernet,
        EthType::Ip,
        src_addr,
        dest_addr,
        opcode,
    )
}
