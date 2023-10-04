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

//! Loopback device.

use alloc::boxed::Box;
use alloc::sync::Arc;
use crabnet::data_link::MacAddr;
use crabnet::network::Ipv4Addr;

use crate::utils::dma::DmaAllocator;

use super::{NetworkDevice, NetworkDriver, RecvPacket};

pub struct Loopback;

impl NetworkDriver for Loopback {
    fn send(&self, _packet: Box<[u8], DmaAllocator>) {
        todo!()
    }

    fn recv(&self) -> RecvPacket {
        todo!()
    }

    fn recv_end(&self, _packet_id: usize) {
        todo!()
    }

    #[inline]
    fn mac(&self) -> MacAddr {
        // TODO: What should this really be?
        MacAddr::NULL
    }
}

lazy_static::lazy_static! {
    pub static ref LOOPBACK: Arc<NetworkDevice> = (|| {
        let device = Arc::new(NetworkDevice::new(Arc::new(Loopback)));

        device.set_ip(Ipv4Addr::LOOPBACK);
        device.set_subnet_mask(Ipv4Addr::new(255, 0, 0, 0));

        device
    })();
}
