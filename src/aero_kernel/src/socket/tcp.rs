use aero_syscall::socket::{MessageFlags, MessageHeader};
use aero_syscall::{InAddr, OpenFlags, SocketAddrInet, AF_INET};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crabnet::network::Ipv4Addr;
use spin::Once;

use crabnet::data_link::{Eth, EthType, MacAddr};
use crabnet::transport::{Tcp, TcpOptions};
use crabnet_tcp::{Address, Error as TcpError, Packet as TcpPacket, State};

use crate::fs::file_table::FileHandle;
use crate::fs::inode::{FileType, INodeInterface, Metadata, PollFlags, PollTable};
use crate::fs::{self, FileSystemError};
use crate::net;
use crate::net::shim::PacketSend;
use crate::net::{tcp, NetworkDevice};
use crate::utils::sync::{Mutex, WaitQueue};

// ./aero.py -- -netdev user,id=mynet0 -device e1000,netdev=mynet0,id=ck_nic0 -object
// filter-dump,id=mynet0,netdev=mynet0,file=qemulog.log

struct DeviceShim(Arc<NetworkDevice>);

impl crabnet_tcp::NetworkDevice for DeviceShim {
    fn ip(&self) -> Ipv4Addr {
        self.0.ip()
    }

    fn send(&self, packet: TcpPacket, _handle: crabnet_tcp::RetransmitHandle) {
        // TODO(andypython): Handle TCP retransmission here.
        let eth = Eth::new(MacAddr::NULL, self.0.mac(), EthType::Ip);
        (eth / packet.ip / packet.tcp / packet.options / packet.payload).send();
    }

    fn remove_retransmit(&self, _seq_number: u32) {
        // TODO(andypython): Handle TCP retransmission here.
    }
}

pub struct TcpSocket {
    tcp: Mutex<Option<crabnet_tcp::Socket<DeviceShim>>>,
    wq: WaitQueue,
    handle: Once<Arc<FileHandle>>,
    sref: Weak<TcpSocket>,
    peer: Once<SocketAddrInet>,
}

impl TcpSocket {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            tcp: Mutex::new(None),
            wq: WaitQueue::new(),
            sref: sref.clone(),
            handle: Once::new(),
            peer: Once::new(),
        })
    }

    pub fn on_packet(&self, tcp: &Tcp, options: TcpOptions, payload: &[u8]) {
        if let Some(socket) = self.tcp.lock_irq().as_mut() {
            // Ignore any invalid TCP options.
            let options = options
                .iter()
                .filter_map(|option| option.ok())
                .collect::<Vec<_>>();

            socket.on_packet(tcp, &options, payload);
            self.wq.notify_all();
        }
    }

    fn sref(&self) -> Arc<TcpSocket> {
        self.sref.upgrade().unwrap()
    }

    /// Returns whether the socket is in non-blocking mode.
    pub fn non_blocking(&self) -> bool {
        self.handle
            .get()
            .map(|handle| handle.flags.read().contains(OpenFlags::O_NONBLOCK))
            .unwrap_or_default()
    }

    pub fn do_recv(&self, buf: &mut [u8]) -> Result<usize, FileSystemError> {
        let mut tcp = self.tcp.lock_irq();
        let socket = tcp.as_mut().ok_or(FileSystemError::NotConnected)?;

        match socket.recv(buf) {
            Ok(bytes_read) => Ok(bytes_read),

            Err(TcpError::WouldBlock) if self.non_blocking() => Err(FileSystemError::WouldBlock),
            Err(TcpError::WouldBlock) => {
                drop(tcp);

                let mut socket = self.wq.block_on(&self.tcp, |tcp| {
                    tcp.as_ref()
                        .map(|socket| !socket.recv_queue.is_empty())
                        .unwrap_or(true)
                })?;

                if let Some(socket) = socket.as_mut() {
                    Ok(socket.recv(buf).unwrap())
                } else {
                    Err(FileSystemError::NotConnected)
                }
            }

            Err(err) => unreachable!("{err:?}"),
        }
    }

    pub fn send(&self, buf: &[u8]) -> Result<usize, FileSystemError> {
        let mut tcp = self.tcp.lock_irq();
        let socket = tcp.as_mut().ok_or(FileSystemError::NotConnected)?;

        let bytes_written = socket.send(buf).unwrap();
        Ok(bytes_written)
    }
}

impl INodeInterface for TcpSocket {
    fn connect(&self, address: super::SocketAddrRef, _length: usize) -> crate::fs::Result<()> {
        {
            let mut tcp = self.tcp.lock_irq();
            assert!(tcp.is_none(), "connect: socket is already initialized");

            let port = tcp::alloc_ephemeral_port(self.sref()).unwrap();

            let addr = address.as_inet().ok_or(FileSystemError::NotSupported)?;
            self.peer.call_once(|| addr.clone());

            let addr = Address::new(port, addr.port(), addr.addr().into());

            let device = Arc::new(DeviceShim(net::default_device()));
            let socket = crabnet_tcp::Socket::connect(device, addr);

            *tcp = Some(socket);
        }

        let _ = self.wq.block_on(&self.tcp, |x| {
            x.as_ref().unwrap().state() == State::Established
        });

        Ok(())
    }

    fn open(
        &self,
        _flags: aero_syscall::OpenFlags,
        handle: Arc<FileHandle>,
    ) -> fs::Result<Option<fs::cache::DirCacheItem>> {
        self.handle.call_once(|| handle);
        Ok(None)
    }

    #[inline]
    fn metadata(&self) -> Result<Metadata, FileSystemError> {
        Ok(Metadata::with_file_type(FileType::Socket))
    }

    #[inline]
    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> Result<usize, FileSystemError> {
        self.do_recv(buf)
    }

    #[inline]
    fn write_at(&self, _offset: usize, buf: &[u8]) -> Result<usize, FileSystemError> {
        self.send(buf)
    }

    fn send(&self, message_hdr: &mut MessageHeader, _flags: MessageFlags) -> fs::Result<usize> {
        let data = message_hdr
            .iovecs()
            .iter()
            .flat_map(|e| e.as_slice())
            .copied()
            .collect::<Vec<_>>();

        let mut tcp = self.tcp.lock_irq();
        let socket = tcp.as_mut().ok_or(FileSystemError::NotSupported)?;

        // TODO: handle fragmentation in crabnet_tcp
        for chunk in data.chunks(1460) {
            socket.send(chunk).expect("failed to send data");
        }

        // -netdev user,id=mynet0,net=192.168.1.0/24,dhcpstart=192.168.1.128,hostfwd=tcp::4444-:80
        // -device e1000,netdev=mynet0,id=ck_nic0 -object
        // filter-dump,id=mynet0,netdev=user,file=qemulog.log

        Ok(data.len())
    }

    fn get_peername(&self) -> fs::Result<super::SocketAddr> {
        if let Some(peer) = self.peer.get() {
            let addr = super::SocketAddr::Inet(peer.clone());
            Ok(addr)
        } else {
            Err(FileSystemError::NotConnected)
        }
    }

    fn get_sockname(&self) -> fs::Result<super::SocketAddr> {
        if let Some(socket) = self.tcp.lock().as_mut() {
            // FIXME:
            let addr = SocketAddrInet {
                family: AF_INET,
                port: socket.addr.src_port.into(),
                sin_addr: InAddr { addr: 0 },
                padding: [0; 8],
            };

            Ok(super::SocketAddr::Inet(addr))
        } else {
            Err(FileSystemError::NotConnected)
        }
    }

    fn recv(&self, message_hdr: &mut MessageHeader, _flags: MessageFlags) -> fs::Result<usize> {
        Ok(message_hdr
            .iovecs_mut()
            .iter_mut()
            .map(|iovec| {
                let iovec = iovec.as_slice_mut();
                self.do_recv(iovec).unwrap()
            })
            .sum::<usize>())
    }

    fn poll(&self, table: Option<&mut PollTable>) -> fs::Result<PollFlags> {
        if let Some(table) = table {
            table.insert(&self.wq);
        }

        let mut flags = PollFlags::empty();
        let mut tcp = self.tcp.lock_irq();

        if let Some(socket) = tcp.as_mut() {
            assert_ne!(socket.state(), State::Closed);

            flags |= PollFlags::OUT;

            if !socket.recv_queue.is_empty() {
                flags |= PollFlags::IN;
            }
        }

        Ok(flags)
    }
}
