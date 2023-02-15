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
pub mod dhcp;
pub mod ethernet;
pub mod ip;
pub mod udp;

pub use ethernet::{Eth, MacAddr};

use crate::{
    mem::paging::VirtAddr,
    userland::{scheduler, task::Task},
};

#[downcastable]
pub trait NetworkDevice: Send + Sync {
    fn send(&self, packet: Packet<Eth>);
    fn recv(&self) -> RecvPacket;
    fn recv_end(&self, packet_id: usize);
    fn mac(&self) -> MacAddr;
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

    fn header(&self) -> &H {
        self.addr().read_mut::<H>().unwrap()
    }

    fn header_mut(&mut self) -> &mut H {
        self.addr().read_mut::<H>().unwrap()
    }
}

static DEVICES: RwLock<Vec<Arc<dyn NetworkDevice>>> = RwLock::new(Vec::new());
static DEFAULT_DEVICE: RwLock<Option<Arc<dyn NetworkDevice>>> = RwLock::new(None);

fn packet_processor_thread() {
    let device = default_device();

    loop {
        log::debug!("bruh!");
        let packet = device.recv();
        log::debug!("{packet:?}");
    }
}

pub fn add_device(device: Arc<dyn NetworkDevice>) {
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

pub fn default_device() -> Arc<dyn NetworkDevice> {
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
    dhcp::init();
}
