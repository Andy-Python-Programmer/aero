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

use aero_syscall::{OpenFlags, SocketAddrUnix, SyscallError, AF_UNIX};

use aero_syscall::socket::{MessageFlags, MessageHeader};

use alloc::collections::VecDeque;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::Once;

use crate::arch::user_copy::UserRef;
use crate::fs;
use crate::fs::cache::DirCacheItem;
use crate::fs::file_table::FileHandle;
use crate::fs::inode::*;

use crate::fs::{FileSystemError, Path};

use crate::mem::paging::VirtAddr;
use crate::utils::sync::{Mutex, WaitQueue};

use super::SocketAddrRef;

fn path_from_unix_sock(address: &SocketAddrUnix) -> fs::Result<&Path> {
    // The abstract namespace socket allows the creation of a socket
    // connection which does not require a path to be created.
    let abstract_namespaced = address.path[0] == 0;
    assert!(!abstract_namespaced);

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

#[derive(Debug, Default)]
pub struct Message {
    data: Vec<u8>,
    // TODO: Keep track of the sender of the message here?
}

impl Message {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

#[derive(Default)]
pub struct MessageQueue {
    messages: VecDeque<Message>,
}

impl MessageQueue {
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        if let Some(message) = self.messages.front_mut() {
            let message_len = message.data.len();
            let size = core::cmp::min(buffer.len(), message_len);

            buffer[..size].copy_from_slice(&message.data[..size]);

            if size < message_len {
                message.data.drain(..size);
                return size;
            }

            self.messages.pop_front();
            size
        } else {
            unreachable!("MessageQueue::read() called when queue is empty");
        }
    }

    pub fn write(&mut self, buffer: &[u8]) {
        let message = Message::new(buffer.to_vec());
        self.messages.push_back(message);
    }
}

pub struct AcceptQueue {
    sockets: VecDeque<Arc<UnixSocket>>,
    backlog: usize,
}

impl AcceptQueue {
    /// # Parameters
    /// * `backlog`: The maximum number of pending connections that the queue can hold.
    pub fn new(backlog: usize) -> Self {
        Self {
            sockets: VecDeque::with_capacity(backlog),
            backlog,
        }
    }

    /// Returns `true` if the queue contains no pending connections.
    pub fn is_empty(&self) -> bool {
        self.sockets.is_empty()
    }

    /// Adds the given socket to the queue. Returns `EAGAIN` if the
    /// queue is full.
    pub fn push(&mut self, socket: Arc<UnixSocket>) -> Result<(), SyscallError> {
        if self.backlog == self.sockets.len() {
            return Err(SyscallError::EAGAIN);
        }

        self.sockets.push_back(socket);
        Ok(())
    }

    /// Removes the first pending connection from the queue and
    /// returns it, or [`None`] if it is empty.
    pub fn pop(&mut self) -> Option<Arc<UnixSocket>> {
        self.sockets.pop_front()
    }

    /// Updates the maximum number of pending connections that the
    /// queue can hold. Returns `EINVAL` if the new backlog is smaller
    /// than the current number of pending connections.
    pub fn set_backlog(&mut self, backlog: usize) -> Result<(), SyscallError> {
        if backlog < self.sockets.len() {
            return Err(SyscallError::EINVAL);
        }

        self.backlog = backlog;
        Ok(())
    }
}

#[derive(Default)]
enum UnixSocketState {
    /// The socket is not connected.
    #[default]
    Disconnected,

    /// The socket is listening for new connections.
    Listening(AcceptQueue),

    /// The socket has connected to a peer.
    Connected(Arc<UnixSocket>),
}

impl UnixSocketState {
    /// Returns `true` if the socket is connected.
    fn is_connected(&self) -> bool {
        matches!(self, Self::Connected(_))
    }

    fn queue(&mut self) -> Option<&mut AcceptQueue> {
        match self {
            Self::Listening(q) => Some(q),
            _ => None,
        }
    }
}

#[derive(Default)]
struct UnixSocketInner {
    /// The address that the socket has been bound to.
    address: Option<SocketAddrUnix>,

    state: UnixSocketState,
}

pub struct UnixSocket {
    inner: Mutex<UnixSocketInner>,
    buffer: Mutex<MessageQueue>,
    wq: WaitQueue,
    weak: Weak<UnixSocket>,
    handle: Once<Arc<FileHandle>>,
}

impl UnixSocket {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak| Self {
            inner: Mutex::new(UnixSocketInner::default()),

            buffer: Mutex::new(MessageQueue::default()),
            wq: WaitQueue::new(),
            weak: weak.clone(),
            handle: Once::new(),
        })
    }

    pub fn connect_pair(a: DirCacheItem, b: DirCacheItem) -> fs::Result<()> {
        let a = a
            .inode()
            .downcast_arc::<UnixSocket>()
            .ok_or(FileSystemError::NotSocket)?;

        let b = b
            .inode()
            .downcast_arc::<UnixSocket>()
            .ok_or(FileSystemError::NotSocket)?;

        a.inner.lock_irq().state = UnixSocketState::Connected(b.clone());
        b.inner.lock_irq().state = UnixSocketState::Connected(a);
        Ok(())
    }

    pub fn sref(&self) -> Arc<Self> {
        self.weak.upgrade().unwrap()
    }

    pub fn is_non_block(&self) -> bool {
        self.handle
            .get()
            .expect("unix: not bound to an fd")
            .flags
            .read()
            .contains(OpenFlags::O_NONBLOCK)
    }
}

impl INodeInterface for UnixSocket {
    fn metadata(&self) -> fs::Result<Metadata> {
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

    fn read_at(&self, _offset: usize, user_buffer: &mut [u8]) -> fs::Result<usize> {
        if self.buffer.lock_irq().is_empty() && self.is_non_block() {
            return Err(FileSystemError::WouldBlock);
        }

        let mut buffer = self.wq.block_on(&self.buffer, |e| !e.is_empty())?;

        let read = buffer.read(user_buffer);
        Ok(read)
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        let inner = self.inner.lock_irq();
        let peer = match inner.state {
            UnixSocketState::Connected(ref peer) => peer,
            _ => return Err(FileSystemError::NotConnected),
        };

        peer.buffer.lock_irq().write(buffer);
        peer.wq.notify_all();

        Ok(buffer.len())
    }

    fn listen(&self, backlog: usize) -> Result<(), SyscallError> {
        let mut inner = self.inner.lock_irq();
        let is_bound = inner.address.is_some();

        match &mut inner.state {
            // We cannot listen on a socket that has not been bound.
            UnixSocketState::Disconnected if is_bound => {
                inner.state = UnixSocketState::Listening(AcceptQueue::new(backlog));
                Ok(())
            }

            UnixSocketState::Listening(queue) => {
                queue.set_backlog(backlog)?;
                Ok(())
            }

            _ => unreachable!(),
        }
    }

    fn bind(&self, address: SocketAddrRef, _length: usize) -> fs::Result<()> {
        let address = address.as_unix().ok_or(FileSystemError::NotSupported)?;
        let path = path_from_unix_sock(address)?;

        if fs::lookup_path(path).is_ok() {
            return Err(FileSystemError::EntryExists);
        }

        let (parent, name) = path.parent_and_basename();
        DirEntry::from_socket_inode(fs::lookup_path(parent)?, String::from(name), self.sref())?;

        let mut inner = self.inner.lock_irq();
        inner.address = Some(address.clone());

        Ok(())
    }

    fn connect(&self, address: SocketAddrRef, _length: usize) -> fs::Result<()> {
        let address = address.as_unix().ok_or(FileSystemError::NotSupported)?;
        let path = path_from_unix_sock(address)?;
        let socket = fs::lookup_path(path)?;

        let target = socket
            .inode()
            .as_unix_socket()?
            .downcast_arc::<UnixSocket>()
            .ok_or(FileSystemError::NotSocket)?;

        let mut itarget = target.inner.lock_irq();

        let queue = match &mut itarget.state {
            UnixSocketState::Listening(queue) => queue,
            _ => return Err(FileSystemError::ConnectionRefused),
        };

        queue.push(self.sref()).unwrap();
        target.wq.notify_all();
        core::mem::drop(itarget); // release the lock

        let _ = self.wq.block_on(&self.inner, |e| e.state.is_connected())?;
        Ok(())
    }

    fn accept(&self, address: Option<(VirtAddr, &mut u32)>) -> fs::Result<Arc<UnixSocket>> {
        let mut inner = self.wq.block_on(&self.inner, |e| {
            e.state.queue().map(|x| !x.is_empty()).unwrap_or(false)
        })?;

        let queue = inner
            .state
            .queue()
            .ok_or(FileSystemError::ConnectionRefused)?;

        let peer = queue.pop().expect("UnixSocket::accept(): backlog is empty");
        let sock = Self::new();

        {
            let mut sock_inner = sock.inner.lock_irq();
            sock_inner.state = UnixSocketState::Connected(peer.clone());
        }

        {
            let mut peer_data = peer.inner.lock_irq();
            peer_data.state = UnixSocketState::Connected(sock.clone());
        }

        if let Some((address, length)) = address {
            let mut address = unsafe { UserRef::new(address) };

            if let Some(paddr) = peer.inner.lock_irq().address.as_ref() {
                *address = paddr.clone();
            } else {
                *address = SocketAddrUnix::default();
                address.family = AF_UNIX;
            }

            *length = core::mem::size_of::<SocketAddrUnix>() as u32;
        }

        peer.wq.notify_all();
        Ok(sock)
    }

    fn recv(&self, header: &mut MessageHeader, flags: MessageFlags) -> fs::Result<usize> {
        assert!(flags.is_empty());

        let inner = self.inner.lock_irq();

        let peer = match &inner.state {
            UnixSocketState::Connected(peer) => peer,
            _ => return Err(FileSystemError::NotConnected),
        };

        if self.buffer.lock_irq().is_empty() && self.is_non_block() {
            return Err(FileSystemError::WouldBlock);
        }

        let mut buffer = self.wq.block_on(&self.buffer, |e| !e.is_empty())?;

        if let Some(addr) = header.name_mut::<SocketAddrUnix>() {
            *addr = peer.inner.lock_irq().address.as_ref().cloned().unwrap();
        }

        Ok(header
            .iovecs_mut()
            .iter_mut()
            .map(|iovec| buffer.read(iovec.as_slice_mut()))
            .sum::<usize>())
    }

    fn poll(&self, table: Option<&mut PollTable>) -> fs::Result<PollFlags> {
        let buffer = self.buffer.lock_irq();
        let inner = self.inner.lock_irq();

        if let Some(e) = table {
            e.insert(&self.wq)
        }

        let mut events = PollFlags::OUT;

        if let UnixSocketState::Listening(queue) = &inner.state {
            if !queue.is_empty() {
                events.insert(PollFlags::IN);
                return Ok(events);
            }
        }

        if !buffer.is_empty() {
            events.insert(PollFlags::IN);
        }

        Ok(events)
    }

    fn shutdown(&self, how: usize) -> fs::Result<()> {
        log::warn!("shutdown how={how}");
        Ok(())
    }
}
