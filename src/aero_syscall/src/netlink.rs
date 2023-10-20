#![allow(non_camel_case_types)]

use static_assertions::const_assert_eq;

const NLMSG_ALIGNTO: u32 = 4;

/// Aligns `len` to the netlink message alignment.
///
/// **Note**: This function only rounds up.
pub const fn nlmsg_align(len: u32) -> u32 {
    (len + NLMSG_ALIGNTO - 1) & !(NLMSG_ALIGNTO - 1)
}

const RTA_ALIGNTO: u32 = 4;

pub const fn rta_align(len: u32) -> u32 {
    (len + RTA_ALIGNTO - 1) & !(RTA_ALIGNTO - 1)
}

pub const fn rta_length(len: u32) -> u32 {
    rta_align(core::mem::size_of::<rtattr>() as u32) + len
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u16)]
pub enum RtAttrType {
    Unspec,
    Dst,
    Src,
    Iif,
    Oif,
    Gateway,
    Priority,
    PrefSrc,
    Metrics,
    Multipath,
    ProtoInfo, // no longer used
    Flow,
    CacheInfo,
    Session, // no longer used
    MpAlgo,  // no longer used
    Table,
    // RtaMARK,
    // RtaMFC_STATS,
    // RtaVIA,
    // RtaNEWDST,
    // RtaPREF,
    // RtaENCAP_TYPE,
    // RtaENCAP,
    // RtaEXPIRES,
    // RtaPAD,
    // RtaUID,
    // RtaTTL_PROPAGATE,
    // RtaIP_PROTO,
    // RtaSPORT,
    // RtaDPORT,
    // RtaNH_ID,
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u16)]
pub enum MessageType {
    Noop,
    Error,
    Done,    // end of a dump
    Overrun, // data lost

    // RTM
    RtmNewLink = 16,
    RtmDelLink,
    RtmGetLink,
    RtmSetLink,
    RtmNewAddr,
    RtmDelAddr,
    RtmGetAddr,
    RtmNewRoute = 24,
    RtmDelRoute,
    RtmGetRoute,
    RtmNewNeigh = 28,
    RtmDelNeigh,
    RtmGetNeigh,
    RtmNewRule = 32,
    RtmDelRule,
    RtmGetRule,
    RtmNewQDisc = 36,
    RtmDelQDisc,
    RtmGetQDisc,
    RtmNewTClass = 40,
    RtmDelTClass,
    RtmGetTClass,
    RtmNewTFilter = 44,
    RtmDelTFilter,
    RtmGetTFilter,
    RtmNewAction = 48,
    RtmDelAction,
    RtmGetAction,
    RtmNewPrefix = 52,
    RtmGetMulticast = 58,
    RtmGetAnyCast = 62,
    RtmNewNeighTbl = 64,
    RtmGetNeighTbl = 66,
    RtmSetNeighTbl = 67,
    RtmNewNdUserOpt = 68,
    RtmNewAddrLabel = 72,
    RtmDelAddrLabel,
    RtmGetAddrLabel,
    RtmGetDcb = 78,
    RtmSetDcb,
    RtmNewNetConf = 80,
    RtmDelNetConf,
    RtmGetNetConf,
    RtmNewMdb = 84,
    RtmDelMdb,
    RtmGetMdb,
    RtmNewNsid = 88,
    RtmDelNsid,
    RtmGetNsid,
    RtmNewStats = 92,
    RtmGetStats = 94,
    RtmSetStats = 95,
    RtmNewCacheReport = 96,
    RtmNewChain = 100,
    RtmDelChain,
    RtmGetChain,
    RtmNewNextHop = 104,
    RtmDelNextHop,
    RtmGetNextHop,
    RtmNewLinkProp = 108,
    RtmDelLinkProp,
    RtmGetLinkProp,
    RtmNewVlan = 112,
    RtmDelVlan,
    RtmGetVlan,
    RtmNewNextHopBucket = 116,
    RtmDelNextHopBucket,
    RtmGetNextHopBucket,
    RtmNewTunnel = 120,
    RtmDelTunnel,
    RtmGetTunnel,

    Unknown = u16::MAX,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct MessageFlags: u16 {
        const REQUEST = 0x1; // It is a request message.
        const MULTI = 0x2; // Multipart message, terminated by NLMSG_DONE.
        const ACK = 0x4; // Reply with ack, with zero or error code.
        const ECHO = 0x8; // Echo this request.
        const DUMP_INTR = 0x10; // Dump was inconsistent due to sequence change.
        const DUMP_FILTERED = 0x20; // Dump was filtered as requested.

        // Modifers to GET request.
        const ROOT = 0x100; // specify tree root.
        const MATCH = 0x200; // return all matching.
        const ATOMIC = 0x400; // atomic GET.
        const DUMP = MessageFlags::ROOT.bits() | MessageFlags::MATCH.bits();
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct sockaddr_nl {
    pub nl_family: u32, // AF_NETLINK
    pub nl_pad: u16,    // zero
    pub nl_pid: u32,    // port ID
    pub nl_groups: u32, // multicast groups mask
}

impl super::SocketAddr for sockaddr_nl {}

/// Fixed format metadata header of Netlink messages.
#[derive(Debug)]
#[repr(C)]
pub struct nlmsghdr {
    /// Length of message including header.
    pub nlmsg_len: u32,
    /// Message content type.
    pub nlmsg_type: MessageType,
    // Additional flags.
    pub nlmsg_flags: MessageFlags,
    /// Sequence number.
    pub nlmsg_seq: u32,
    /// Sending process port ID.
    pub nlmsg_pid: u32,
}

const_assert_eq!(core::mem::size_of::<nlmsghdr>(), 16);

/// General form of address family dependent message.
#[derive(Debug)]
#[repr(C)]
pub struct rtgenmsg {
    pub rtgen_family: u8,
}

const_assert_eq!(core::mem::size_of::<rtgenmsg>(), 1);

#[repr(C)]
#[derive(Debug)]
pub struct rtmsg {
    pub rtm_family: u8,
    pub rtm_dst_len: u8,
    pub rtm_src_len: u8,
    pub rtm_tos: u8,
    pub rtm_table: u8,
    pub rtm_protocol: u8,
    pub rtm_scope: u8,
    pub rtm_type: u8,
    pub rtm_flags: u32,
}

const_assert_eq!(core::mem::size_of::<rtmsg>(), 12);

// FIXME(andypython): This should be an enum.
//
// Reserved table identifiers.
pub const RT_TABLE_UNSPEC: u8 = 0;
// User defined values.
pub const RT_TABLE_DEFAULT: u8 = 253;
pub const RT_TABLE_MAIN: u8 = 254;
pub const RT_TABLE_LOCAL: u8 = 255;

// Generic structure for encapsulation of optional route information. It is reminiscent of sockaddr,
// but with sa_family replaced with attribute type.
pub struct rtattr {
    pub rta_len: u16,
    pub rta_type: RtAttrType,
}
