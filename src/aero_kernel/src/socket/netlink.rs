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

//! Netlink Sockets
//!
//! Netlink sockets are used for inter-process communication (IPC) between both the kernel and
//! userspace processes, and between different userspace processes, in a way similar to the Unix
//! domain sockets.
//!
//! Netlink is designed and used for transferring miscellaneous networking information between the
//! kernel space and userspace processes. Networking utilities, such as the `iproute2` family use
//! Netlink to communicate with the kernel from userspace.

use aero_syscall::netlink::{MessageFlags, MessageType, RtAttrType};
use aero_syscall::socket::{self, MessageHeader};
use aero_syscall::{netlink, AF_INET, AF_NETLINK, AF_UNSPEC};
use alloc::sync::Arc;
use alloc::vec::Vec;
use crabnet::network::Ipv4Addr;

use crate::fs;
use crate::fs::inode::{FileType, INodeInterface, Metadata, PollFlags, PollTable};
use crate::utils::sync::{Mutex, WaitQueue};

use super::SocketAddrRef;

// TODO(andypython): can we use crabnet to construct netlink packets(?)
struct NetlinkBuilder {
    buffer: Vec<u8>,
}

impl NetlinkBuilder {
    fn new() -> Self {
        // crate::scheduler::get_scheduler().for_each_task(|task| {
        //     if task
        //         .executable
        //         .lock()
        //         .as_ref()
        //         .map(|x| x.absolute_path_str().contains("kit"))
        //         .unwrap_or_default()
        //     {
        //         task.enable_systrace();
        //     }
        // });

        Self { buffer: Vec::new() }
    }

    fn header(&mut self, header: &netlink::nlmsghdr) {
        self.buffer.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<netlink::nlmsghdr>(),
            )
        });

        self.buffer_align();
    }

    fn message(&mut self, message: &netlink::rtmsg) {
        self.buffer.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                &message as *const _ as *const u8,
                core::mem::size_of::<netlink::rtmsg>(),
            )
        });

        self.buffer_align();
    }

    fn rtattr<T>(&mut self, ty: RtAttrType, data: T) {
        let rta_len = netlink::rta_length(core::mem::size_of::<T>() as u32);

        let attr = netlink::rtattr {
            rta_len: rta_len.try_into().unwrap(),
            rta_type: ty,
        };

        let padding = rta_len as usize - core::mem::size_of::<T>();

        self.buffer.extend_from_slice(unsafe {
            core::slice::from_raw_parts(
                &attr as *const _ as *const u8,
                core::mem::size_of::<netlink::rtattr>(),
            )
        });

        self.buffer.extend_from_slice(unsafe {
            core::slice::from_raw_parts(&data as *const _ as *const u8, core::mem::size_of::<T>())
        });

        self.buffer.resize(self.buffer.len() + padding, 0);
        self.buffer_align();
    }

    /// Aligns the buffer to the netlink message alignment.
    fn buffer_align(&mut self) {
        let aligned_len = netlink::nlmsg_align(self.buffer.len() as u32);

        // Resize will not truncate the buffer since [`nlmsg_align`]` only rounds up.
        self.buffer.resize(aligned_len as usize, 0);
    }

    fn build(self) -> Vec<u8> {
        let mut buffer = self.buffer;

        let msg_len = buffer.len();
        let msg_hdr = unsafe { &mut *buffer.as_mut_ptr().cast::<netlink::nlmsghdr>() };

        msg_hdr.nlmsg_len = msg_len as u32;
        buffer
    }
}

pub struct NetLinkSocket {
    recv_queue: Mutex<Vec<Vec<u8>>>,
    recv_wq: WaitQueue,
}

impl NetLinkSocket {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            recv_queue: Mutex::new(Vec::new()),
            recv_wq: WaitQueue::new(),
        })
    }

    fn validate_message<'a, T>(header: &'a netlink::nlmsghdr, payload: &'a [u8]) -> &'a T {
        let hdr_len = core::mem::size_of::<netlink::nlmsghdr>() as u32;
        let msg_len = core::mem::size_of::<T>() as u32;

        // TODO(andypython): send an error message instead of panicking.
        assert!(header.nlmsg_len == hdr_len + msg_len);

        // FIXME(andypython): use bytemuck to cast the payload to T.
        unsafe { &*payload.as_ptr().cast::<T>() }
    }

    fn send_route_packet(&self, header: &netlink::nlmsghdr) {
        let mut builder = NetlinkBuilder::new();

        builder.header(&netlink::nlmsghdr {
            nlmsg_type: MessageType::RtmNewRoute,
            nlmsg_flags: MessageFlags::MULTI,
            nlmsg_seq: header.nlmsg_seq,
            nlmsg_pid: 0,
            nlmsg_len: 0,
        });

        builder.message(&netlink::rtmsg {
            rtm_family: AF_INET as u8,
            rtm_dst_len: 0, // FIXME
            rtm_src_len: 0,
            rtm_tos: 0,
            rtm_table: netlink::RT_TABLE_MAIN,

            rtm_protocol: 0,
            rtm_scope: 0,
            rtm_type: 0,
            rtm_flags: 0,
        });

        builder.rtattr(RtAttrType::Table, netlink::RT_TABLE_MAIN);

        // Qemu SLIRP
        builder.rtattr(RtAttrType::Dst, Ipv4Addr::new(10, 0, 2, 15));
        builder.rtattr(RtAttrType::Gateway, Ipv4Addr::new(10, 0, 2, 2));

        self.recv_queue.lock().push(builder.build());
        self.recv_wq.notify();
    }

    fn get_route(&self, header: &netlink::nlmsghdr, payload: &[u8]) {
        assert!(header
            .nlmsg_flags
            .contains(MessageFlags::REQUEST | MessageFlags::DUMP));

        let payload = Self::validate_message::<netlink::rtgenmsg>(header, payload);
        let rtgen_family = payload.rtgen_family as u32;

        assert!(rtgen_family == AF_UNSPEC || rtgen_family == AF_NETLINK);

        self.send_route_packet(header);
    }
}

impl INodeInterface for NetLinkSocket {
    fn metadata(&self) -> fs::Result<Metadata> {
        Ok(Metadata::with_file_type(FileType::Socket))
    }

    fn bind(&self, _addr: SocketAddrRef, _len: usize) -> fs::Result<()> {
        Ok(())
    }

    fn connect(&self, _address: SocketAddrRef, _length: usize) -> fs::Result<()> {
        unimplemented!()
    }

    fn read_at(&self, _offset: usize, _buffer: &mut [u8]) -> fs::Result<usize> {
        unimplemented!()
    }

    fn write_at(&self, _offset: usize, _buffer: &[u8]) -> fs::Result<usize> {
        unimplemented!()
    }

    fn recv(
        &self,
        message_hdr: &mut MessageHeader,
        flags: socket::MessageFlags,
    ) -> fs::Result<usize> {
        // FIXME(andypython): All of the message header and iovec logic should be moved to
        // syscall::net::recvmsg() instead.

        if let Some(addr) = message_hdr.name_mut::<netlink::sockaddr_nl>() {
            *addr = netlink::sockaddr_nl {
                nl_family: AF_NETLINK,
                nl_pad: 0,
                nl_pid: 0,
                nl_groups: 0,
            };
        }

        let mut queue = self
            .recv_wq
            .block_on(&self.recv_queue, |queue| !queue.is_empty())?;

        let mut bytes_copied = 0;
        dbg!(message_hdr.iovecs_mut());
        let mut iovecs = message_hdr.iovecs_mut().to_vec();

        while let Some(data) = queue.pop() {
            if let Some((index, ref mut iovec)) = iovecs
                .iter_mut()
                .enumerate()
                .find(|(_, iovec)| iovec.len() >= data.len())
            {
                let iovec = iovec.as_slice_mut();
                assert!(iovec.len() >= data.len());

                let copy = core::cmp::min(iovec.len(), data.len());
                iovec[..copy].copy_from_slice(&data[..copy]);

                bytes_copied += copy;
                iovecs.remove(index);
            } else if flags.contains(socket::MessageFlags::TRUNC) && bytes_copied == 0 {
                message_hdr.flags = socket::MessageFlags::TRUNC.bits() as i32;
                return Ok(data.len());
            } else {
                unimplemented!()
            }
        }

        Ok(bytes_copied)
    }

    fn send(
        &self,
        message_hdr: &mut MessageHeader,
        flags: socket::MessageFlags,
    ) -> fs::Result<usize> {
        log::warn!("netlink::send(flags={flags:?})");

        // FIXME(andypython): figure out the message header stuff...
        let data = message_hdr
            .iovecs()
            .iter()
            .flat_map(|e| e.as_slice())
            .copied()
            .collect::<Vec<_>>();

        let hdr_size = core::mem::size_of::<netlink::nlmsghdr>();

        let mut offset = 0;

        while offset + hdr_size <= data.len() {
            let header = unsafe { &*(data.as_ptr().cast::<netlink::nlmsghdr>().byte_add(offset)) };
            let payload = &data[offset + hdr_size..];

            match header.nlmsg_type {
                MessageType::Done => break,
                MessageType::Error => {
                    unimplemented!("netlink::send: error message received");
                }

                MessageType::RtmGetRoute => self.get_route(header, payload),

                ty => unimplemented!("netlink::send: unknown message type {ty:?}"),
            }

            offset += header.nlmsg_len as usize;
        }

        Ok(data.len())
    }

    fn poll(&self, _table: Option<&mut PollTable>) -> fs::Result<PollFlags> {
        unimplemented!()
    }

    fn get_peername(&self) -> fs::Result<super::SocketAddr> {
        unimplemented!()
    }

    fn get_sockname(&self) -> fs::Result<super::SocketAddr> {
        // TODO(andypython): fill in `nl_groups` and `nl_pid`.
        Ok(super::SocketAddr::Netlink(netlink::sockaddr_nl {
            nl_family: AF_NETLINK,
            nl_pad: 0,
            nl_pid: 0,
            nl_groups: 0,
        }))
    }
}
