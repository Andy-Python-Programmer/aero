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

use aero_syscall::{IpProtocol, SocketAddrInet, SocketType, SyscallError};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::fs::inode::{FileType, INodeInterface, Metadata};
use crate::fs::{self, FileSystemError};
use crate::net::ip::Ipv4Addr;
use crate::net::udp::{self, Udp, UdpHandler};
use crate::net::{Packet, PacketHeader, PacketTrait};
use crate::utils::sync::Mutex;

#[derive(Default)]
enum SocketState {
    /// The socket is not connected.
    #[default]
    Disconnected,
    Connected(SocketAddrInet),
}

#[derive(Default)]
struct InetSocketInner {
    /// The address that the socket has been bound to.
    address: Option<SocketAddrInet>,
    state: SocketState,
}

pub struct InetSocket {
    typ: SocketType,
    protocol: IpProtocol,
    inner: Mutex<InetSocketInner>,

    sref: Weak<Self>,
}

impl InetSocket {
    pub fn new(typ: SocketType, protocol: IpProtocol) -> Result<Arc<Self>, SyscallError> {
        Ok(Arc::new_cyclic(|sref| Self {
            typ,
            protocol,

            inner: Mutex::new(Default::default()),
            sref: sref.clone(),
        }))
    }

    fn sref(&self) -> Arc<Self> {
        self.sref.upgrade().unwrap()
    }

    fn set_state(&self, state: SocketState) {
        self.inner.lock_irq().state = state;
    }

    fn set_addr(&self, addr: SocketAddrInet) {
        self.inner.lock_irq().address = Some(addr);
    }

    fn src_port(&self) -> Option<u16> {
        self.inner
            .lock_irq()
            .address
            .as_ref()
            .map(|e| e.port.to_native())
    }

    fn dest(&self) -> SocketAddrInet {
        match &self.inner.lock_irq().state {
            SocketState::Connected(addr) => addr.clone(),
            _ => unreachable!(),
        }
    }
}

impl INodeInterface for InetSocket {
    fn metadata(&self) -> fs::Result<fs::inode::Metadata> {
        Ok(Metadata {
            id: 0,
            file_type: FileType::Socket,
            size: 0,
            children_len: 0,
        })
    }

    fn bind(&self, address: super::SocketAddr, _length: usize) -> fs::Result<()> {
        let address = address.as_inet().ok_or(FileSystemError::NotSupported)?;

        match (self.typ, self.protocol) {
            (SocketType::Dgram, IpProtocol::Default) => {
                self.set_addr(address.clone());
                udp::bind(address.port.to_native(), self.sref());
                Ok(())
            }

            _ => unreachable!(),
        }
    }

    fn connect(&self, address: super::SocketAddr, _length: usize) -> fs::Result<()> {
        let address = address.as_inet().ok_or(FileSystemError::NotSupported)?;

        match (self.typ, self.protocol) {
            (SocketType::Dgram, IpProtocol::Default) => {
                let host_addr = Ipv4Addr::new(address.sin_addr.addr.to_be_bytes());
                udp::connect(host_addr, address.port.to_native());

                self.set_state(SocketState::Connected(address.clone()));
                Ok(())
            }

            _ => unreachable!(),
        }
    }

    fn send(&self, message_hdr: &mut aero_syscall::socket::MessageHeader) -> fs::Result<usize> {
        let name = message_hdr
            .name_mut::<SocketAddrInet>()
            .cloned()
            .unwrap_or_else(|| self.dest());

        let dest_port = name.port.to_native();
        let dest_ip = Ipv4Addr::new(name.addr());

        let src_port = self.src_port().unwrap_or_else(|| {
            udp::alloc_ephemeral_port(self.sref()).expect("inet: out of ephemeral ports")
        });

        let data = message_hdr
            .iovecs()
            .iter()
            .map(|e| e.as_slice())
            .flatten()
            .copied()
            .collect::<Vec<_>>();

        let mut packet = Packet::<Udp>::create(src_port, dest_port, data.len(), dest_ip);

        let dest = packet.as_slice_mut();
        dest.copy_from_slice(data.as_slice());

        packet.send();
        Ok(data.len())
    }
}

impl UdpHandler for InetSocket {}
