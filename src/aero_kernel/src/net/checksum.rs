use simple_endian::BigEndian;

use super::ip::{self, Ipv4Addr};

#[repr(C, packed)]
pub struct PseudoHeader {
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    reserved: u8,
    ty: ip::Type,
    size: BigEndian<u16>,
}

impl PseudoHeader {
    pub fn new(ip_hdr: &ip::Header) -> PseudoHeader {
        let len = ip_hdr.length;
        PseudoHeader {
            src_ip: ip_hdr.src_ip,
            dst_ip: ip_hdr.dest_ip,
            reserved: 0,
            ty: ip_hdr.protocol,
            size: BigEndian::from(len.to_native() - core::mem::size_of::<ip::Header>() as u16),
        }
    }
}

/// Compute the 32-bit internet checksum for `data`.
fn calculate_checksum(data: &[u8]) -> u32 {
    let bytes = unsafe {
        core::slice::from_raw_parts(
            data.as_ptr() as *const BigEndian<u16>,
            data.len() / core::mem::size_of::<u16>(),
        )
    };

    let mut sum = 0;

    for i in 0..(data.len() / 2) {
        sum += bytes[i].to_native() as u32
    }

    // Add left-over byte, if any.
    if data.len() % 2 == 1 {
        sum += ((*data.last().unwrap()) as u32) << 8;
    }

    sum
}

/// Folds the 32-bit sum (`sum`) to 16 bits in the network byte order.
pub fn make(mut sum: u32) -> BigEndian<u16> {
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    BigEndian::from(!(sum as u16))
}

/// Combine several RFC 1071 compliant checksums.
pub fn make_combine(a: &[u32]) -> BigEndian<u16> {
    make(a.iter().sum())
}

/// Compute the internet checksum for `value`.
pub fn calculate<T: Sized>(value: &T) -> u32 {
    let bytes = unsafe {
        core::slice::from_raw_parts(value as *const _ as *const u8, core::mem::size_of::<T>())
    };
    calculate_checksum(bytes)
}

/// Compute the internet checksum for `value` of `size`.
pub fn calculate_with_len<T: ?Sized>(value: &T, size: usize) -> u32 {
    let bytes = unsafe { core::slice::from_raw_parts(value as *const _ as *const u8, size) };
    calculate_checksum(bytes)
}
