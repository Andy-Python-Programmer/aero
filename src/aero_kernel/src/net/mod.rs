// Copyright (C) 2021-2024 The Aero Project Developers.
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

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use crabnet::transport::TcpOptions;
use spin::RwLock;

pub mod arp;
pub mod loopback;
pub mod tcp;
pub mod udp;

use crate::userland::scheduler;
use crate::userland::task::Task;
use crate::utils::dma::DmaAllocator;

use crabnet::data_link::MacAddr;
use crabnet::network::Ipv4Addr;

#[downcastable]
pub trait NetworkDriver: Send + Sync {
    fn send(&self, packet: Box<[u8], DmaAllocator>);
    fn recv(&self) -> RecvPacket;
    fn recv_end(&self, packet_id: usize);
    fn mac(&self) -> MacAddr;
}

#[derive(Default)]
struct Metadata {
    ip: Ipv4Addr,
    #[allow(dead_code)]
    subnet_mask: Ipv4Addr,
    default_gateway: Ipv4Addr,
}

// FIXME(andypython): This is very inefficient. We store the driver as an Arc<dyn NetworkDriver> and
// the device with metadata as an Arc<NetworkDevice>. Two heap allocations for nothing, bruh
// moments.
pub struct NetworkDevice {
    driver: Arc<dyn NetworkDriver>,
    metadata: RwLock<Metadata>,
}

impl NetworkDevice {
    pub fn new(driver: Arc<dyn NetworkDriver>) -> Self {
        // FIXME(andy): DHCPD should handle static IP assignment.
        //
        // https://wiki.qemu.org/Documentation/Networking
        let metadata = Metadata {
            ip: Ipv4Addr::new(192, 168, 100, 0),
            // What should the default be? Also this should really be handled inside dhcpd.
            default_gateway: Ipv4Addr::new(10, 0, 2, 2),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            ..Default::default()
        };

        Self {
            driver,
            metadata: RwLock::new(metadata),
        }
    }

    pub fn set_ip(&self, ip: Ipv4Addr) {
        self.metadata.write().ip = ip;
    }

    pub fn set_subnet_mask(&self, mask: Ipv4Addr) {
        self.metadata.write().ip = mask;
    }

    pub fn ip(&self) -> Ipv4Addr {
        self.metadata.read().ip
    }

    pub fn subnet_mask(&self) -> Ipv4Addr {
        self.metadata.read().subnet_mask
    }

    #[inline]
    pub fn default_gateway(&self) -> Ipv4Addr {
        self.metadata.read().default_gateway
    }
}

impl core::ops::Deref for NetworkDevice {
    type Target = Arc<dyn NetworkDriver>;

    fn deref(&self) -> &Self::Target {
        &self.driver
    }
}

#[derive(Debug)]
pub struct RecvPacket<'a> {
    pub packet: &'a [u8],
    pub id: usize,
}

impl<'a> Drop for RecvPacket<'a> {
    fn drop(&mut self) {
        default_device().recv_end(self.id)
    }
}

static DEVICES: RwLock<Vec<Arc<NetworkDevice>>> = RwLock::new(Vec::new());
static DEFAULT_DEVICE: RwLock<Option<Arc<NetworkDevice>>> = RwLock::new(None);

fn packet_processor_thread() {
    use crabnet::data_link::{Arp, Eth, EthType};
    use crabnet::network::{Ipv4, Ipv4Type};
    use crabnet::transport::{Tcp, Udp};
    use crabnet::PacketParser;

    let device = default_device();

    loop {
        let packet = device.recv();

        let mut parser = PacketParser::new(packet.packet);
        let eth = parser.next::<Eth>();

        match eth.typ() {
            EthType::Ip => {
                let ip = parser.next::<Ipv4>();

                match ip.protocol() {
                    Ipv4Type::Udp => {
                        let udp = parser.next::<Udp>();
                        let size = ip.payload_len() as usize - core::mem::size_of::<Udp>();

                        let payload = &parser.payload()[..size];
                        udp::on_packet(udp, payload);
                    }

                    Ipv4Type::Tcp => {
                        let tcp = parser.next::<Tcp>();
                        let size = ip.payload_len() as usize - tcp.header_size() as usize;
                        let options = parser.next::<TcpOptions>();
                        let payload = &parser.payload()[..size];

                        tcp::on_packet(tcp, options, payload)
                    }
                }
            }

            EthType::Arp => {
                arp::do_recv(parser.next::<Arp>());
            }
        }
    }
}

pub fn add_device(device: NetworkDevice) {
    let device = Arc::new(device);
    DEVICES.write().push(device.clone());

    let mut default_device = DEFAULT_DEVICE.write();
    if default_device.is_none() {
        *default_device = Some(device);
    }

    scheduler::get_scheduler().register_task(Task::new_kernel(packet_processor_thread, true));
}

pub fn has_default_device() -> bool {
    DEFAULT_DEVICE.read().as_ref().is_some()
}

pub fn default_device() -> Arc<NetworkDevice> {
    DEFAULT_DEVICE
        .read()
        .as_ref()
        .expect("net: no devices found")
        .clone()
}

// Initialize the networking stack.
pub fn init() {
    if !has_default_device() {
        // No network devices are avaliable.
        return;
    }

    DEVICES.write().push(loopback::LOOPBACK.clone());
    arp::init();
    log::info!("net::arp: initialized cache");
}

pub type RawPacket = Box<[u8], DmaAllocator>;

pub mod shim {
    use crate::net::{self, arp};
    use crate::utils::dma::DmaAllocator;

    use crabnet::data_link::{Arp, Eth, EthType, MacAddr};
    use crabnet::network::Ipv4;
    use crabnet::{IntoBoxedBytes, Protocol, Stacked};

    pub trait PacketSend {
        fn send(self);
    }

    // Deref<T> for Stacked<T, U> where T: Stacked?
    //
    // TODO(andypython): Can all of the packet send impls be refactored?
    impl<T: Protocol, U: Protocol> PacketSend for Stacked<Stacked<Stacked<Eth, Ipv4>, T>, U> {
        fn send(mut self) {
            let device = net::default_device();

            let eth = &mut self.upper.upper.upper;
            let ip = &self.upper.upper.lower;

            let mut dest_ip = ip.dest_ip();

            if !dest_ip.is_broadcast() && !dest_ip.is_same_subnet(device.ip(), device.subnet_mask())
            {
                dest_ip = device.default_gateway();
            }

            eth.src_mac = device.mac();

            if let Some(addr) = arp::get(dest_ip) {
                eth.dest_mac = addr;
                device.send(self.into_boxed_bytes_in(DmaAllocator));
            } else {
                arp::request_ip(dest_ip, self.into_boxed_bytes_in(DmaAllocator));
            }
        }
    }

    impl<T: Protocol, U: Protocol, S: Protocol> PacketSend
        for Stacked<Stacked<Stacked<Stacked<Eth, Ipv4>, T>, U>, S>
    {
        fn send(mut self) {
            let device = net::default_device();

            let eth = &mut self.upper.upper.upper.upper;
            let ip = &self.upper.upper.upper.lower;

            let mut dest_ip = ip.dest_ip();

            if !dest_ip.is_broadcast() && !dest_ip.is_same_subnet(device.ip(), device.subnet_mask())
            {
                dest_ip = device.default_gateway();
            }

            eth.src_mac = device.mac();

            if let Some(addr) = arp::get(dest_ip) {
                eth.dest_mac = addr;
                device.send(self.into_boxed_bytes_in(DmaAllocator));
            } else {
                arp::request_ip(dest_ip, self.into_boxed_bytes_in(DmaAllocator));
            }
        }
    }

    impl PacketSend for Arp {
        fn send(self) {
            let device = net::default_device();

            let eth = Eth::new(MacAddr::NULL, MacAddr::BROADCAST, EthType::Arp)
                .set_dest_mac(self.dest_mac())
                .set_src_mac(device.mac());

            device.send((eth / self).into_boxed_bytes_in(DmaAllocator));
        }
    }

    //     struct DefaultDevice;

    // impl<A: Allocator> NetworkDevice<A> for DefaultDevice {
    //     fn send_bytes(&self, bytes: Box<[u8], A>) {
    //         panic!("Sending {} bytes", bytes.len());
    //     }
    // }

    // pub trait NetworkDevice<A: Allocator = Global> {
    //     fn send_bytes(&self, bytes: Box<[u8], A>);
    // }

    // pub trait SendablePacket<A: Allocator = Global>
    // where
    //     Self: Sized + IntoBoxedBytes<A>,
    // {
    //     #[inline]
    //     fn send(self) {
    //         DefaultDevice.send_bytes(self.into_boxed_bytes())
    //     }

    //     #[inline]
    //     fn send_in<T: NetworkDevice<A>>(self, device: &T) {
    //         device.send_bytes(self.into_boxed_bytes())
    //     }
    // }

    // impl<T: IntoBoxedBytes> SendablePacket for T {}
}
