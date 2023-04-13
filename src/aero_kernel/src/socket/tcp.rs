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
use spin::Once;

use crate::fs::cache::DirCacheItem;
use crate::fs::file_table::FileHandle;
use crate::fs::inode::{FileType, INodeInterface, Metadata};
use crate::fs::{self, FileSystemError};
use crate::net::tcp::{self, TcpHandler};

pub struct TcpSocket {
    sref: Weak<Self>,
    handle: Once<Arc<FileHandle>>,
}

impl TcpSocket {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            handle: Once::new(),
            sref: sref.clone(),
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
        let _address = address.as_inet().ok_or(FileSystemError::NotSupported)?;
        tcp::alloc_ephemeral_port(self.sref());

        Ok(())
    }

    fn recv(
        &self,
        _message_hdr: &mut aero_syscall::socket::MessageHeader,
        _flags: aero_syscall::socket::MessageFlags,
    ) -> fs::Result<usize> {
        todo!()
    }

    fn send(
        &self,
        _message_hdr: &mut aero_syscall::socket::MessageHeader,
        _flags: aero_syscall::socket::MessageFlags,
    ) -> fs::Result<usize> {
        todo!()
    }

    fn poll(&self, _table: Option<&mut fs::inode::PollTable>) -> fs::Result<fs::inode::PollFlags> {
        todo!()
    }
}

impl TcpHandler for TcpSocket {
    fn recv(&self, _packet: crate::net::Packet<crate::net::tcp::Tcp>) {
        todo!()
    }
}
