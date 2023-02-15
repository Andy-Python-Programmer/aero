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

// Dynamic Host Configuration Protocol

use byteorder::{ByteOrder, NetworkEndian};
use simple_endian::BigEndian;

use super::ip::Ipv4Addr;
use super::udp::{self, Udp};
use super::{ConstPacketKind, Packet, PacketDownHierarchy, PacketHeader, PacketUpHierarchy};

const DHCP_XID: u32 = 0x43424140;

#[repr(u8)]
enum DhcpType {
    BootRequest = 1u8.swap_bytes(),
    BootReply = 2u8.swap_bytes(),
}

#[repr(u8)]
enum HType {
    Ethernet = 1u8.swap_bytes(),
}

#[repr(C, packed)]
struct Header {
    op: DhcpType,
    htype: HType,
    hlen: BigEndian<u8>,
    hops: BigEndian<u8>,
    xid: BigEndian<u32>,
    seconds: BigEndian<u16>,
    flags: BigEndian<u16>,
    client_ip: Ipv4Addr,
    your_ip: Ipv4Addr,
    server_ip: Ipv4Addr,
    gateway_ip: Ipv4Addr,
    client_hw_addr: [u8; 16],
    server_name: [u8; 64],
    file: [u8; 128],
    options: [u8; 64],
}

impl Header {
    fn options_mut(&mut self) -> OptionsWriter<'_> {
        OptionsWriter::new(&mut self.options)
    }
}

pub struct Dhcp;

impl ConstPacketKind for Dhcp {
    const HSIZE: usize = core::mem::size_of::<Header>();
}

impl PacketUpHierarchy<Dhcp> for Packet<udp::Udp> {}
impl PacketHeader<Header> for Packet<Dhcp> {
    fn send(&self) {
        self.downgrade().send() // send the UDP packet
    }
}

#[repr(u8)]
enum MessageType {
    Discover = 1u8.swap_bytes(),
}

#[repr(u8)]
enum DhcpOption {
    HostName = 12,
    MessageType = 53,
    ParameterRequestList = 55,
    ClientIdentifier = 61,
    End = 255,
}

struct OptionsWriter<'a>(&'a mut [u8]);

impl<'a> OptionsWriter<'a> {
    fn new(options: &'a mut [u8]) -> Self {
        options.fill(0);
        Self(options).set_magic_cookie()
    }

    fn insert(&mut self, kind: DhcpOption, data: &'_ [u8]) {
        let total_len = 2 + data.len();

        assert!(data.len() < u8::MAX as _);
        assert!(self.0.len() > total_len);

        let (buf, rest) = core::mem::take(&mut self.0).split_at_mut(total_len);
        self.0 = rest;

        buf[0] = kind as u8;
        buf[1] = data.len() as _;
        buf[2..].copy_from_slice(data);
    }

    fn insert_padding(&mut self, size: usize) {
        let (buf, rest) = core::mem::take(&mut self.0).split_at_mut(size);
        self.0 = rest;

        buf.fill(0);
    }

    fn set_magic_cookie(mut self) -> Self {
        let (buf, rest) = core::mem::take(&mut self.0).split_at_mut(core::mem::size_of::<u32>());

        // The first four octets of the 'options' field of the DHCP message
        // contain the (decimal) values 99, 130, 83 and 99, respectively.
        //
        // CC: (https://www.rfc-editor.org/rfc/rfc2131#section-3)
        NetworkEndian::write_u32(buf, 0x63825363);
        self.0 = rest;
        self
    }

    fn set_message_type(mut self, typ: MessageType) -> Self {
        self.insert(DhcpOption::MessageType, &[typ as u8]);
        self
    }

    fn set_parameter_request_list(mut self) -> Self {
        // TODO: Take all of the request flags as an argument.
        self.insert(
            DhcpOption::ParameterRequestList,
            &[
                1,  // Subnet Mask
                3,  // Router
                15, // Domain Name
                6,  // Domain Server
            ],
        );
        self
    }

    fn set_client_identifier(mut self) -> Self {
        let mac = super::default_device().mac();

        let mut data = [0; 7];
        data[0] = HType::Ethernet as u8;
        data[1..].copy_from_slice(mac.0.as_slice());

        self.insert(DhcpOption::ClientIdentifier, data.as_slice());
        self
    }

    fn set_host_name(mut self, name: &str) -> Self {
        self.insert(DhcpOption::HostName, name.as_bytes());
        self.insert_padding(1); // null-terminator
        self
    }
}

impl<'a> Drop for OptionsWriter<'a> {
    fn drop(&mut self) {
        self.insert(DhcpOption::End, &[]);
    }
}

pub fn init() {
    // Send DHCP discover message.
    let mut packet = Packet::<Udp>::create(68, 67, Dhcp::HSIZE, Ipv4Addr::BROADCAST).upgrade();
    let header = packet.header_mut();

    header.htype = HType::Ethernet;
    header.hlen = BigEndian::<u8>::from(6);
    header.hops = BigEndian::<u8>::from(0);
    header.xid = BigEndian::<u32>::from(DHCP_XID);
    header.seconds = BigEndian::<u16>::from(0);
    {
        // Set the HW address.
        let mac = super::default_device().mac();
        header.client_hw_addr[0..6].copy_from_slice(mac.0.as_slice());
        header.client_hw_addr[6..].fill(0);
    }
    header.server_name.fill(0);
    header.file.fill(0);
    header.options.fill(0);

    header.op = DhcpType::BootRequest;
    header.flags = BigEndian::from(0x8000); // broadcast
    header.client_ip = Ipv4Addr::EMPTY;
    header.your_ip = Ipv4Addr::EMPTY;
    header.server_ip = Ipv4Addr::EMPTY;
    header.gateway_ip = Ipv4Addr::EMPTY;

    let _ = header
        .options_mut()
        .set_message_type(MessageType::Discover)
        .set_host_name("Aero")
        .set_client_identifier()
        .set_parameter_request_list();

    packet.send()
}
