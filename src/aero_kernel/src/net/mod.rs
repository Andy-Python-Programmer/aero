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

use core::marker::PhantomData;

use alloc::{sync::Arc, vec::Vec};
use spin::RwLock;

pub mod arp;
mod checksum;
pub mod ethernet;
pub mod ip;
pub mod tcp;
pub mod udp;

pub use ethernet::{Eth, MacAddr};

use crate::{
    mem::paging::VirtAddr,
    userland::{scheduler, task::Task},
};

use self::ip::Ipv4Addr;

#[downcastable]
pub trait NetworkDriver: Send + Sync {
    fn send(&self, packet: Packet<Eth>);
    fn recv(&self) -> RecvPacket;
    fn recv_end(&self, packet_id: usize);
    fn mac(&self) -> MacAddr;
}

#[derive(Default)]
struct Metadata {
    ip: Ipv4Addr,
    #[allow(dead_code)]
    subnet_mask: Ipv4Addr,
}

pub struct NetworkDevice {
    driver: Arc<dyn NetworkDriver>,
    metadata: RwLock<Metadata>,
}

impl NetworkDevice {
    pub fn new(driver: Arc<dyn NetworkDriver>) -> Self {
        // FIXME(andy): DHCPD should handle static IP assignment.
        let mut metadata = Metadata::default();
        metadata.ip = Ipv4Addr::new([192, 168, 122, 0]);

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
}

impl core::ops::Deref for NetworkDevice {
    type Target = Arc<dyn NetworkDriver>;

    fn deref(&self) -> &Self::Target {
        &self.driver
    }
}

#[derive(Debug)]
pub struct RecvPacket {
    pub packet: Packet<Eth>,
    pub id: usize,
}

impl Drop for RecvPacket {
    fn drop(&mut self) {
        default_device().recv_end(self.id)
    }
}

pub trait PacketKind {}

pub trait ConstPacketKind: PacketKind {
    const HSIZE: usize;
}

impl<T: ConstPacketKind> PacketKind for T {}

impl<U, D> PacketDownHierarchy<D> for Packet<U>
where
    U: PacketKind,
    D: ConstPacketKind,
    Packet<D>: PacketUpHierarchy<U>,
{
}

pub trait PacketBaseTrait {
    fn addr(&self) -> VirtAddr;
    fn len(&self) -> usize;
}

pub trait PacketTrait: PacketBaseTrait {
    fn header_size(&self) -> usize;

    // TODO: Rename as_slice{_mut} to payload{_mut}?
    fn as_slice_mut(&mut self) -> &mut [u8] {
        let hsize = self.header_size();

        let start = self.addr() + hsize;
        let size = self.len() - hsize;

        unsafe { core::slice::from_raw_parts_mut(start.as_mut_ptr(), size) }
    }

    fn as_slice(&self) -> &[u8] {
        let hsize = self.header_size();

        let start = self.addr() + hsize;
        let size = self.len() - hsize;

        unsafe { core::slice::from_raw_parts(start.as_ptr(), size) }
    }
}

impl<T: ConstPacketKind> PacketTrait for Packet<T> {
    fn header_size(&self) -> usize {
        T::HSIZE
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Packet<T: PacketKind> {
    pub addr: VirtAddr,
    pub len: usize,
    _phantom: PhantomData<T>,
}

impl<T: PacketKind> PacketBaseTrait for Packet<T> {
    fn addr(&self) -> VirtAddr {
        self.addr
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl<T: PacketKind> Packet<T> {
    pub fn new(addr: VirtAddr, len: usize) -> Packet<T> {
        Packet::<T> {
            addr,
            len,
            _phantom: PhantomData::default(),
        }
    }
}

pub trait PacketUpHierarchy<B: PacketKind>: PacketTrait {
    fn upgrade(&self) -> Packet<B> {
        let header_size = self.header_size();
        Packet::<B>::new(self.addr() + header_size, self.len() - header_size)
    }
}

pub trait PacketDownHierarchy<B: ConstPacketKind>: PacketBaseTrait {
    fn downgrade(&self) -> Packet<B> {
        let header_size = B::HSIZE;
        Packet::<B>::new(self.addr() - header_size, self.len() + header_size)
    }
}

pub trait PacketHeader<H>: PacketBaseTrait {
    fn send(&self);
    fn recv(&self);

    fn header(&self) -> &H {
        self.addr().read_mut::<H>().unwrap()
    }

    fn header_mut(&mut self) -> &mut H {
        self.addr().read_mut::<H>().unwrap()
    }
}

static DEVICES: RwLock<Vec<Arc<NetworkDevice>>> = RwLock::new(Vec::new());
static DEFAULT_DEVICE: RwLock<Option<Arc<NetworkDevice>>> = RwLock::new(None);

fn packet_processor_thread() {
    let device = default_device();

    loop {
        let packet = device.recv();
        packet.packet.recv();
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

    arp::init();
    log::info!("net::arp: initialized cache");
}
