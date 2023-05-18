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

pub mod tcp;
pub mod udp;
pub mod unix;

use aero_syscall::prelude::IfReq;
use aero_syscall::*;

use crate::mem::paging::VirtAddr;

#[derive(Debug)]
pub enum SocketAddr<'a> {
    Unix(&'a SocketAddrUnix),
    INet(&'a SocketAddrInet),
}

impl<'a> SocketAddr<'a> {
    pub fn from_family(address: VirtAddr, family: u32) -> Result<Self, SyscallError> {
        match family {
            AF_UNIX => Ok(SocketAddr::Unix(address.read_mut::<SocketAddrUnix>()?)),
            AF_INET => Ok(SocketAddr::INet(address.read_mut::<SocketAddrInet>()?)),

            _ => Err(SyscallError::EINVAL),
        }
    }

    pub fn from_ifreq(ifreq: &IfReq) -> Result<Self, SyscallError> {
        // SAFETY???
        let family = unsafe { ifreq.data.addr.sa_family };
        SocketAddr::from_family(
            VirtAddr::new(&unsafe { ifreq.data.addr } as *const _ as _),
            family,
        )
    }

    /// Converts the socket address into a unix socket address. Returns [`None`] if
    /// the address is not a unix socket address.
    pub fn as_unix(&self) -> Option<&'a SocketAddrUnix> {
        match self {
            SocketAddr::Unix(address) => Some(address),
            _ => None,
        }
    }

    pub fn as_inet(&self) -> Option<&'a SocketAddrInet> {
        match self {
            SocketAddr::INet(addr) => Some(addr),
            _ => None,
        }
    }
}
