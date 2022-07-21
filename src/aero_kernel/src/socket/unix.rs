/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
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

use aero_syscall::SocketAddrUnix;

use aero_syscall::socket::MessageHeader;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::fs;
use crate::fs::inode::{DirEntry, FileType, INodeInterface, Metadata, PollFlags, PollTable};
use crate::fs::{FileSystemError, Path, Result};
use crate::utils::buffer::Buffer;
use crate::utils::sync::{BlockQueue, Mutex};

use super::SocketAddr;

fn path_from_unix_sock<'sock>(address: &'sock SocketAddrUnix) -> Result<&'sock Path> {
    // The abstract namespace socket allows the creation of a socket
    // connection which does not require a path to be created.
    let abstrat_namespaced = address.path[0] == 0;
    assert!(!abstrat_namespaced);

    let path_len = address
        .path
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(address.path.len());

    let path_str = core::str::from_utf8(&address.path[..path_len])
        .ok()
        .ok_or(FileSystemError::InvalidPath)?;

    Ok(Path::new(path_str))
}

#[derive(Default)]
struct UnixSocketBacklog {
    backlog: Option<Vec<Arc<UnixSocket>>>,
}

impl UnixSocketBacklog {
    pub fn push(&mut self, socket: Arc<UnixSocket>) {
        if let Some(ref mut backlog) = self.backlog {
            assert!(backlog.len() != backlog.capacity());
            backlog.push(socket);
        }
    }

    pub fn len(&self) -> usize {
        self.backlog.as_ref().map(|e| e.len()).unwrap_or_default()
    }

    pub fn pop(&mut self) -> Option<Arc<UnixSocket>> {
        self.backlog.as_mut().map(|e| e.pop()).unwrap_or_default()
    }

    pub fn update_capacity(&mut self, capacity: usize) {
        assert!(
            self.backlog.is_none(),
            "UnixSocket::listen() has already been called"
        );

        self.backlog = Some(Vec::with_capacity(capacity));
    }
}

#[derive(Default)]
struct UnixSocketInner {
    backlog: UnixSocketBacklog,
    listening: bool,
    peer: Option<Arc<UnixSocket>>,
    connected: bool,
}

pub struct UnixSocket {
    inner: Mutex<UnixSocketInner>,
    buffer: Mutex<Buffer>,
    wq: BlockQueue,
    weak: Weak<UnixSocket>,
}

impl UnixSocket {
    pub fn new(peer: Option<Arc<UnixSocket>>) -> Arc<Self> {
        Arc::new_cyclic(|weak| Self {
            inner: Mutex::new(UnixSocketInner {
                peer,
                ..Default::default()
            }),

            buffer: Mutex::new(Buffer::new()),
            wq: BlockQueue::new(),
            weak: weak.clone(),
        })
    }

    pub fn sref(&self) -> Arc<Self> {
        self.weak.upgrade().unwrap()
    }
}

impl INodeInterface for UnixSocket {
    fn read_at(&self, _offset: usize, user_buffer: &mut [u8]) -> Result<usize> {
        let mut buffer = self.wq.block_on(&self.buffer, |lock| lock.has_data())?;

        let read = buffer.read_data(user_buffer);
        Ok(read)
    }

    fn write_at(&self, offset: usize, buffer: &[u8]) -> Result<usize> {
        let inner = self.inner.lock_irq();

        // TODO: Remove the unwrap and return an error instead.
        let peer = inner
            .peer
            .as_ref()
            .expect("UnixSocket::write_at(): socket not connected!");

        let result = offset + peer.buffer.lock_irq().write_data(buffer);
        peer.wq.notify_complete();

        Ok(result)
    }

    fn metadata(&self) -> Result<Metadata> {
        Ok(Metadata {
            id: 0,
            file_type: FileType::Socket,
            size: 0,
            children_len: 0,
        })
    }

    fn bind(&self, address: SocketAddr, _length: usize) -> Result<()> {
        let address = address.as_unix().ok_or(FileSystemError::NotSupported)?;
        let path = path_from_unix_sock(address)?;

        // ensure that the provided path is not already in use.
        if fs::lookup_path(path).is_ok() {
            return Err(FileSystemError::EntryExists);
        }

        let (parent, name) = path.parent_and_basename();

        // create the socket inode.
        DirEntry::from_socket_inode(fs::lookup_path(parent)?, String::from(name), self.sref())?;

        Ok(())
    }

    fn connect(&self, address: SocketAddr, _length: usize) -> Result<()> {
        let address = address.as_unix().ok_or(FileSystemError::NotSupported)?;
        let path = path_from_unix_sock(address)?;
        let socket = fs::lookup_path(path)?;

        let target = socket
            .inode()
            .as_unix_socket()?
            .downcast_arc::<UnixSocket>()
            .ok_or(FileSystemError::NotSocket)?; // NOTE: the provided socket was not a unix socket.

        let mut itarget = target.inner.lock_irq();

        // ensure that the target socket is listening for new connections.
        if !itarget.listening {
            return Err(FileSystemError::ConnectionRefused);
        }

        itarget.backlog.push(self.sref());
        target.wq.notify_complete();
        core::mem::drop(itarget); // release the lock

        let _ = self.wq.block_on(&self.inner, |e| e.connected);
        Ok(())
    }

    fn listen(&self, backlog: usize) -> Result<()> {
        let mut this = self.inner.lock_irq();

        this.backlog.update_capacity(backlog);
        this.listening = true;

        Ok(())
    }

    fn accept(&self, _address: &mut SocketAddr) -> Result<Arc<UnixSocket>> {
        if !self.inner.lock_irq().listening {
            return Err(FileSystemError::ConnectionRefused);
        }

        let mut this = self.wq.block_on(&self.inner, |e| e.backlog.len() != 0)?;

        let peer = this
            .backlog
            .pop()
            .expect("UnixSocket::accept(): backlog is empty");

        let sock = Self::new(Some(peer.clone()));

        {
            let mut sock_inner = sock.inner.lock_irq();
            sock_inner.connected = true;
        }

        {
            let mut peer_data = peer.inner.lock_irq();
            peer_data.peer = Some(sock.clone());
            peer_data.connected = true;
        }

        peer.wq.notify_complete();
        Ok(sock)
    }

    fn recv(&self, _message_header: &mut MessageHeader) -> Result<()> {
        Ok(())
    }

    fn poll(&self, table: Option<&mut PollTable>) -> Result<PollFlags> {
        table.map(|e| e.insert(&self.wq));

        let mut events = PollFlags::OUT;
        let sock_data = self.inner.lock_irq();

        if self.buffer.lock_irq().has_data() || sock_data.backlog.len() > 0 {
            events.insert(PollFlags::IN);
        }

        Ok(events)
    }
}
