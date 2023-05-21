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

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Once;

use crate::fs::cache::DirCacheItem;
use crate::fs::file_table::FileHandle;
use crate::fs::inode::{FileType, INodeInterface, Metadata, PollFlags};
use crate::fs::{self, FileSystemError};
use crate::net::ip::Ipv4Addr;
use crate::net::tcp::{self, Tcp, TcpFlags, TcpHandler};
use crate::net::{Packet, PacketHeader, PacketTrait};
use crate::utils::sync::{Mutex, WaitQueue};

/// TCP Stream
#[derive(Default)]
struct Stream {
    buffer: Vec<u8>,
}

impl Stream {
    fn write(&mut self, buffer: &[u8]) {
        self.buffer.extend_from_slice(buffer);
    }

    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let size = buffer.len().min(self.buffer.len());
        let target = self.buffer.drain(..size).collect::<Vec<_>>();

        buffer[..size].copy_from_slice(target.as_slice());
        size
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[derive(Default)]
struct TransmissionControl {
    /// Sequence number of the next byte to be sent.
    send_next: u32,
    recv_next: u32,
}

#[derive(Default, PartialEq, Eq, Debug)]
enum State {
    #[default]
    Closed,
    SynSent,
    Established,
}

#[derive(Default)]
struct TcpData {
    control: TransmissionControl,
    state: State,

    src_port: u16,
    dest_port: u16,
    target: Ipv4Addr,

    stream: Stream,
}

impl TcpData {
    fn make_packet(&self, size: usize, flags: TcpFlags) -> Packet<Tcp> {
        let mut packet = Packet::<Tcp>::create(self.src_port, self.dest_port, size, self.target);
        let header = packet.header_mut();

        header.set_sequence_number(self.control.send_next);
        header.set_window(u16::MAX);
        header.set_flags(flags);

        packet
    }

    fn make_ack_packet(&self, size: usize) -> Packet<Tcp> {
        let mut packet = self.make_packet(size, TcpFlags::empty());
        let header = packet.header_mut();

        header.set_ack_number(self.control.recv_next);
        packet
    }

    fn send_packet(&mut self, packet: Packet<Tcp>) {
        self.control.send_next = self.control.send_next.wrapping_add(packet.ack_len());
        packet.send();
    }

    fn send_sync(&mut self) {
        self.send_packet(self.make_packet(0, TcpFlags::SYN));
        self.state = State::SynSent;
    }

    fn recv(&mut self, packet: Packet<Tcp>) {
        let header = packet.header();

        match self.state {
            State::SynSent => {
                assert!(header.flags().contains(TcpFlags::ACK | TcpFlags::SYN));
                self.state = State::Established;
            }

            State::Established => {
                if !packet.as_slice().is_empty() {
                    let data = packet.as_slice();
                    self.stream.write(data);
                } else if header.flags().contains(TcpFlags::FIN) {
                    todo!()
                } else {
                    log::trace!("[ TCP ] Connection Established!");
                    return;
                }
            }

            State::Closed => unreachable!(),
        }

        self.control.recv_next = header.sequence_number().wrapping_add(packet.ack_len());
        self.send_packet(self.make_ack_packet(0));
    }
}

pub struct TcpSocket {
    sref: Weak<Self>,
    data: Mutex<TcpData>,
    handle: Once<Arc<FileHandle>>,
    wq: WaitQueue,
}

impl TcpSocket {
    const MAX_MTU: usize = 1460;

    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            handle: Once::new(),
            sref: sref.clone(),
            data: Mutex::new(TcpData::default()),
            wq: WaitQueue::new(),
        })
    }

    fn sref(&self) -> Arc<Self> {
        self.sref.upgrade().unwrap()
    }
}

impl INodeInterface for TcpSocket {
    fn metadata(&self) -> fs::Result<fs::inode::Metadata> {
        Ok(Metadata {
            id: 0,
            file_type: FileType::Socket,
            size: 0,
            children_len: 0,
        })
    }

    fn open(
        &self,
        _flags: aero_syscall::OpenFlags,
        handle: Arc<FileHandle>,
    ) -> fs::Result<Option<DirCacheItem>> {
        self.handle.call_once(|| handle);
        Ok(None)
    }

    fn bind(&self, _address: super::SocketAddr, _length: usize) -> fs::Result<()> {
        todo!()
    }

    fn connect(&self, address: super::SocketAddr, _length: usize) -> fs::Result<()> {
        let address = address.as_inet().ok_or(FileSystemError::NotSupported)?;
        let port = tcp::alloc_ephemeral_port(self.sref()).unwrap();

        let mut inner = self.data.lock_irq();
        inner.src_port = port;
        inner.dest_port = address.port();
        inner.target = Ipv4Addr::new(address.addr());

        inner.send_sync();
        Ok(())
    }

    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        let mut data = self
            .wq
            .block_on(&self.data, |e| e.state == State::Established)?;

        assert!(!data.stream.is_empty());
        Ok(data.stream.read(buffer))
    }

    fn recv(
        &self,
        message_hdr: &mut aero_syscall::socket::MessageHeader,
        _flags: aero_syscall::socket::MessageFlags,
    ) -> fs::Result<usize> {
        let mut data = self.data.lock_irq();
        assert!(!data.stream.is_empty());

        Ok(message_hdr
            .iovecs_mut()
            .iter_mut()
            .map(|iovec| {
                let iovec = iovec.as_slice_mut();
                data.stream.read(iovec)
            })
            .sum::<usize>())
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        let mut data = self
            .wq
            .block_on(&self.data, |e| e.state == State::Established)?;

        for chunk in buffer.chunks(Self::MAX_MTU) {
            let mut packet = data.make_ack_packet(chunk.len());
            packet.as_slice_mut().copy_from_slice(chunk);
            data.send_packet(packet);
        }

        Ok(buffer.len())
    }

    fn send(
        &self,
        message_hdr: &mut aero_syscall::socket::MessageHeader,
        _flags: aero_syscall::socket::MessageFlags,
    ) -> fs::Result<usize> {
        let data = message_hdr
            .iovecs()
            .iter()
            .flat_map(|e| e.as_slice())
            .copied()
            .collect::<Vec<_>>();

        let mut inner = self.data.lock_irq();

        for chunk in data.chunks(Self::MAX_MTU) {
            let mut packet = inner.make_ack_packet(chunk.len());
            packet.as_slice_mut().copy_from_slice(chunk);
            inner.send_packet(packet);
        }

        Ok(data.len())
    }

    fn poll(&self, _table: Option<&mut fs::inode::PollTable>) -> fs::Result<PollFlags> {
        let mut flags = PollFlags::empty();
        let data = self.data.lock_irq();

        if data.state == State::Closed {
            return Ok(flags);
        }

        flags |= PollFlags::OUT;

        if !data.stream.is_empty() {
            flags |= PollFlags::IN;
        }

        Ok(flags)
    }
}

impl TcpHandler for TcpSocket {
    fn recv(&self, packet: Packet<Tcp>) {
        self.data.lock_irq().recv(packet);
    }
}
