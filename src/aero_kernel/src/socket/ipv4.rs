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

//! IPv4 or `AF_RAW` sockets.

use alloc::sync::Arc;

use aero_syscall::prelude::{IfReq, SIOCGIFINDEX};

use crate::arch::user_copy::UserRef;

use crate::fs::inode::INodeInterface;
use crate::fs::Result;

use crate::mem::paging::VirtAddr;

pub struct Ipv4Socket {}

impl Ipv4Socket {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }
}

impl INodeInterface for Ipv4Socket {
    fn ioctl(&self, command: usize, arg: usize) -> Result<usize> {
        match command {
            SIOCGIFINDEX => {
                let mut ifreq = unsafe { UserRef::<IfReq>::new(VirtAddr::new(arg as _)) };

                let name = ifreq.name().unwrap();
                assert!(name == "eth0");

                ifreq.data.ifindex = 1; // FIXME: Fill the actual interface index
                Ok(0)
            }

            _ => unimplemented!(),
        }
    }
}
