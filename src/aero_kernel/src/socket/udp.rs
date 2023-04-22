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

use aero_syscall::prelude::{IfReq, SIOCGIFHWADDR, SIOCGIFINDEX, SIOCSIFADDR, SIOCSIFNETMASK};
use aero_syscall::socket::{MessageFlags, MessageHeader};
use aero_syscall::{OpenFlags, SocketAddrInet};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Once;

use crate::fs::cache::DirCacheItem;
use crate::fs::file_table::FileHandle;
use crate::fs::inode::{FileType, INodeInterface, Metadata, PollFlags};
use crate::fs::{self, FileSystemError};
use crate::mem::paging::VirtAddr;
use crate::net::ip::Ipv4Addr;
use crate::net::udp::{self, Udp, UdpHandler};
use crate::net::{self, MacAddr, Packet, PacketHeader, PacketTrait};
use crate::utils::sync::{Mutex, WaitQueue};

use super::SocketAddr;

#[derive(Default)]
enum SocketState {
    /// The socket is not connected.
    #[default]
    Disconnected,
    Connected(SocketAddrInet),
}

#[derive(Default)]
struct UdpSocketInner {
    /// The address that the socket has been bound to.
    address: Option<SocketAddrInet>,
    state: SocketState,
    incoming: Vec<Packet<Udp>>,
}

pub struct UdpSocket {
    inner: Mutex<UdpSocketInner>,
    wq: WaitQueue,
    handle: Once<Arc<FileHandle>>,

    sref: Weak<Self>,
}

impl UdpSocket {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            wq: WaitQueue::new(),
            handle: Once::new(),

            inner: Mutex::new(Default::default()),
            sref: sref.clone(),
        })
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

    pub fn is_non_block(&self) -> bool {
        self.handle
            .get()
            .expect("inet: not bound to an fd")
            .flags
            .read()
            .contains(OpenFlags::O_NONBLOCK)
    }
}

impl INodeInterface for UdpSocket {
    fn open(
        &self,
        _flags: aero_syscall::OpenFlags,
        handle: Arc<FileHandle>,
    ) -> fs::Result<Option<DirCacheItem>> {
        self.handle.call_once(|| handle);
        Ok(None)
    }

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

        self.set_addr(address.clone());
        udp::bind(address.port.to_native(), self.sref());
        Ok(())
    }

    fn connect(&self, address: super::SocketAddr, _length: usize) -> fs::Result<()> {
        let address = address.as_inet().ok_or(FileSystemError::NotSupported)?;

        let host_addr = Ipv4Addr::new(address.sin_addr.addr.to_be_bytes());
        udp::connect(host_addr, address.port.to_native());

        self.set_state(SocketState::Connected(address.clone()));
        Ok(())
    }

    fn send(&self, message_hdr: &mut MessageHeader, _flags: MessageFlags) -> fs::Result<usize> {
        let name = message_hdr
            .name_mut::<SocketAddrInet>()
            .cloned()
            .unwrap_or_else(|| self.dest());

        let dest_port = name.port.to_native();
        let dest_ip = Ipv4Addr::new(name.addr());

        let src_port;

        if let Some(port) = self.src_port() {
            src_port = port;
        } else {
            src_port = udp::alloc_ephemeral_port(self.sref()).ok_or(FileSystemError::WouldBlock)?;
            log::debug!("Inet::send(): allocated ephemeral port {}", src_port);
        }

        // FIXME: loopback
        if dest_ip == Ipv4Addr::new([127, 0, 0, 1]) {
            return Err(FileSystemError::NotSupported);
        }

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

    fn recv(&self, message_hdr: &mut MessageHeader, _flags: MessageFlags) -> fs::Result<usize> {
        // assert!(flags.is_empty());

        if self.inner.lock_irq().incoming.is_empty() && self.is_non_block() {
            return Err(FileSystemError::WouldBlock);
        }

        let mut this = self.wq.block_on(&self.inner, |e| !e.incoming.is_empty())?;
        let packet = this.incoming.pop().expect("recv: someone was greedy");

        let mut data = packet.as_slice().to_vec();

        Ok(message_hdr
            .iovecs_mut()
            .iter_mut()
            .map(|iovec| {
                let iovec = iovec.as_slice_mut();
                let size = core::cmp::min(iovec.len(), data.len());
                iovec[..size].copy_from_slice(&data.drain(..size).collect::<Vec<_>>());
                size
            })
            .sum::<usize>())
    }

    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            SIOCGIFINDEX => {
                let ifreq = VirtAddr::new(arg as _).read_mut::<IfReq>()?;

                let name = ifreq.name().unwrap();
                assert!(name == "eth0");

                ifreq.data.ifindex = 1; // FIXME: Fill the actual interface index
                Ok(0)
            }

            SIOCGIFHWADDR => {
                let ifreq = VirtAddr::new(arg as _).read_mut::<IfReq>()?;

                let name = ifreq.name().ok_or(FileSystemError::InvalidPath)?;
                assert!(name == "eth0");

                let hwaddr = unsafe {
                    core::slice::from_raw_parts_mut(
                        ifreq.data.addr.sa_data.as_mut_ptr(),
                        MacAddr::ADDR_SIZE,
                    )
                };

                let mac_addr = net::default_device().mac();
                hwaddr.copy_from_slice(&mac_addr.0.as_slice());
                Ok(0)
            }

            SIOCSIFADDR => {
                let ifreq = VirtAddr::new(arg as _).read_mut::<IfReq>()?;
                let socket = SocketAddr::from_ifreq(ifreq)
                    .map_err(|_| FileSystemError::NotSupported)?
                    .as_inet()
                    .ok_or(FileSystemError::NotSupported)?;

                let name = ifreq.name().ok_or(FileSystemError::InvalidPath)?;

                // FIXME:
                assert!(name == "eth0");

                let device = net::default_device();
                device.set_ip(Ipv4Addr::new(socket.addr()));
                Ok(0)
            }

            SIOCSIFNETMASK => {
                let ifreq = VirtAddr::new(arg as _).read_mut::<IfReq>()?;
                let socket = SocketAddr::from_ifreq(ifreq)
                    .map_err(|_| FileSystemError::NotSupported)?
                    .as_inet()
                    .ok_or(FileSystemError::NotSupported)?;

                let name = ifreq.name().ok_or(FileSystemError::InvalidPath)?;

                // FIXME:
                assert!(name == "eth0");

                let device = net::default_device();
                device.set_subnet_mask(Ipv4Addr::new(socket.addr()));

                Ok(0)
            }

            _ => unreachable!("inet::ioctl(): unknown command {command}"),
        }
    }

    fn poll(&self, table: Option<&mut fs::inode::PollTable>) -> fs::Result<PollFlags> {
        if let Some(table) = table {
            table.insert(&self.wq);
        }

        let mut flags = PollFlags::OUT;

        if !self.inner.lock_irq().incoming.is_empty() {
            flags |= PollFlags::IN;
        }

        Ok(flags)
    }
}

impl UdpHandler for UdpSocket {
    fn recv(&self, packet: Packet<Udp>) {
        self.inner.lock_irq().incoming.push(packet);
        self.wq.notify_all();
    }
}
