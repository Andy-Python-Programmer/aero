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

#![no_std]
// #![feature(decl_macro)]
// cc <https://github.com/bitflags/bitflags/issues/110>
#![allow(clippy::bad_bit_mask)]

#[macro_use]
extern crate num_derive;

pub mod consts;
pub mod netlink;
pub mod signal;
pub mod socket;
pub mod syscall;
pub mod time;

pub type Result<T> = core::result::Result<T, SyscallError>;

use byte_endian::BigEndian;

pub use crate::syscall::*;

pub mod prelude {
    pub use crate::consts::*;
    pub use crate::syscall::*;

    pub use crate::SyscallError;
}

bitflags::bitflags! {
    pub struct MMapProt: usize {
        const PROT_READ = 0x1;
        const PROT_WRITE = 0x2;
        const PROT_EXEC = 0x4;
        const PROT_NONE = 0x0;
    }
}

bitflags::bitflags! {
    pub struct MMapFlags: usize {
        const MAP_PRIVATE = 0x1;
        const MAP_SHARED = 0x2;
        const MAP_FIXED = 0x4;
        const MAP_ANONYOMUS = 0x8;
    }
}

bitflags::bitflags! {
    pub struct OpenFlags: usize {
        // reserve 3 bits for the access mode
        const O_ACCMODE =  0x0007;
        const O_EXEC    =  1;
        const O_RDONLY  =  2;
        const O_RDWR    =  3;
        const O_SEARCH  =  4;
        const O_WRONLY  =  5;

        // these flags get their own bit
        const O_APPEND    = 0x000008;
        const O_CREAT     = 0x000010;
        const O_DIRECTORY = 0x000020;
        const O_EXCL      = 0x000040;
        const O_NOCTTY    = 0x000080;
        const O_NOFOLLOW  = 0x000100;
        const O_TRUNC     = 0x000200;
        const O_NONBLOCK  = 0x000400;
        const O_DSYNC     = 0x000800;
        const O_RSYNC     = 0x001000;
        const O_SYNC      = 0x002000;
        const O_CLOEXEC   = 0x004000;
        const O_PATH      = 0x008000;
        const O_LARGEFILE = 0x010000;
        const O_NOATIME   = 0x020000;
        const O_ASYNC     = 0x040000;
        const O_TMPFILE   = 0x080000;
        const O_DIRECT    = 0x100000;
    }
}

bitflags::bitflags! {
    pub struct WaitPidFlags: usize {
        const WNOHANG    = 1;
        const WUNTRACED  = 2;
        const WSTOPPED   = 2;
        const WEXITED    = 4;
        const WCONTINUED = 8;
        const WNOWAIT    = 0x01000000;
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(isize)]
#[allow(clippy::enum_clike_unportable_variant)]
pub enum SyscallError {
    EDOM = 1,
    EILSEQ = 2,
    ERANGE = 3,

    E2BIG = 1001,
    EACCES = 1002,
    EADDRINUSE = 1003,
    EADDRNOTAVAIL = 1004,
    EAFNOSUPPORT = 1005,
    EAGAIN = 1006,
    EALREADY = 1007,
    EBADF = 1008,
    EBADMSG = 1009,
    EBUSY = 1010,
    ECANCELED = 1011,
    ECHILD = 1012,
    ECONNABORTED = 1013,
    ECONNREFUSED = 1014,
    ECONNRESET = 1015,
    EDEADLK = 1016,
    EDESTADDRREQ = 1017,
    EDQUOT = 1018,
    EEXIST = 1019,
    EFAULT = 1020,
    EFBIG = 1021,
    EHOSTUNREACH = 1022,
    EIDRM = 1023,
    EINPROGRESS = 1024,
    EINTR = 1025,
    EINVAL = 1026,
    EIO = 1027,
    EISCONN = 1028,
    EISDIR = 1029,
    ELOOP = 1030,
    EMFILE = 1031,
    EMLINK = 1032,
    EMSGSIZE = 1034,
    EMULTIHOP = 1035,
    ENAMETOOLONG = 1036,
    ENETDOWN = 1037,
    ENETRESET = 1038,
    ENETUNREACH = 1039,
    ENFILE = 1040,
    ENOBUFS = 1041,
    ENODEV = 1042,
    ENOENT = 1043,
    ENOEXEC = 1044,
    ENOLCK = 1045,
    ENOLINK = 1046,
    ENOMEM = 1047,
    ENOMSG = 1048,
    ENOPROTOOPT = 1049,
    ENOSPC = 1050,
    ENOSYS = 1051,
    ENOTCONN = 1052,
    ENOTDIR = 1053,
    ENOTEMPTY = 1054,
    ENOTRECOVERABLE = 1055,
    ENOTSOCK = 1056,
    ENOTSUP = 1057,
    ENOTTY = 1058,
    ENXIO = 1059,
    EOPNOTSUPP = 1060,
    EOVERFLOW = 1061,
    EOWNERDEAD = 1062,
    EPERM = 1063,
    EPIPE = 1064,
    EPROTO = 1065,
    EPROTONOSUPPORT = 1066,
    EPROTOTYPE = 1067,
    EROFS = 1068,
    ESPIPE = 1069,
    ESRCH = 1070,
    ESTALE = 1071,
    ETIMEDOUT = 1072,
    ETXTBSY = 1073,
    EXDEV = 1075,
    ENODATA = 1076,
    ETIME = 1077,
    ENOKEY = 1078,
    ESHUTDOWN = 1079,
    EHOSTDOWN = 1080,
    EBADFD = 1081,
    ENOMEDIUM = 1082,
    ENOTBLK = 1083,

    Unknown = isize::MAX,
}

#[derive(Debug)]
#[repr(usize)]
pub enum SysFileType {
    Unknown = 0,
    Fifo = 1,
    CharDevice = 2,
    Directory = 4,
    BlockDevice = 6,
    File = 8,
    Symlink = 10,
    Socket = 12,
}

#[repr(C, packed)]
pub struct SysDirEntry {
    pub inode: usize,
    pub offset: usize,
    pub reclen: usize,
    pub file_type: usize,
    pub name: [u8; 0],
}

#[repr(C)]
#[derive(Debug)]
pub struct Utsname {
    pub sysname: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
    pub domainname: [u8; 65],
}

impl Utsname {
    pub fn name(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.sysname) }
    }

    pub fn nodename(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.nodename) }
    }

    pub fn release(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.release) }
    }

    pub fn version(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.version) }
    }

    pub fn machine(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.machine) }
    }
}

impl Default for Utsname {
    fn default() -> Self {
        Self {
            sysname: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
            domainname: [0; 65],
        }
    }
}

#[derive(Default, Clone, Debug)]
#[repr(C)]
pub struct TimeSpec {
    pub tv_sec: isize,
    pub tv_nsec: isize,
}

#[repr(usize)]
#[derive(Debug)]
pub enum SeekWhence {
    SeekCur = 1,
    SeekEnd = 2,
    SeekSet = 3,
}

impl From<usize> for SeekWhence {
    fn from(x: usize) -> Self {
        match x {
            1 => SeekWhence::SeekCur,
            2 => SeekWhence::SeekEnd,
            3 => SeekWhence::SeekSet,
            _ => panic!("invalid seek_whence: {}", x),
        }
    }
}

pub const TIOCGWINSZ: usize = 0x5413;
pub const TIOCSWINSZ: usize = 0x5414;
pub const TCGETS: usize = 0x5401;
pub const TCSETSW: usize = 0x5403;
pub const TCSETSF: usize = 0x5404;
pub const TIOCSCTTY: usize = 0x540e;
pub const TIOCNOTTY: usize = 0x5422;
pub const TIOCGPGRP: usize = 0x540f;

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

// indices for the c_cc array in struct termios
//
// abis/linux/termios.h
pub const VINTR: usize = 0;
pub const VQUIT: usize = 1;
pub const VERASE: usize = 2;
pub const VKILL: usize = 3;
pub const VEOF: usize = 4;
pub const VTIME: usize = 5;
pub const VMIN: usize = 6;
pub const VSWTC: usize = 7;
pub const VSTART: usize = 8;
pub const VSTOP: usize = 9;
pub const VSUSP: usize = 10;
pub const VEOL: usize = 11;
pub const VREPRINT: usize = 12;
pub const VDISCARD: usize = 13;
pub const VWERASE: usize = 14;
pub const VLNEXT: usize = 15;
pub const VEOL2: usize = 16;

bitflags::bitflags! {
    #[derive(Default)]
    pub struct TermiosIFlag: u32 {
        const BRKINT = 0o000002;
        const ICRNL  = 0o000400;
        const IGNBRK = 0o000001;
        const IGNCR  = 0o000200;
        const IGNPAR = 0o000004;
        const INLCR  = 0o000100;
        const INPCK  = 0o000020;
        const ISTRIP = 0o000040;
        const IXANY  = 0o004000;
        const IXOFF  = 0o010000;
        const IXON   = 0o002000;
        const PARMRK = 0o000010;
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct TermiosLFlag: u32 {
        const ECHO    = 0x8;
        const ECHOE   = 0x10;
        const ECHOK   = 0x20;
        const ECHONL  = 0x40;
        const ICANON  = 0x2;
        const IEXTEN  = 0x8000;
        const ISIG    = 0x1;
        const NOFLSH  = 0x80;
        const TOSTOP  = 0x100;
        const ECHOPRT = 0x400;
        // options/posix/include/termios.h
        const ECHOCTL = 0o001000;
        const FLUSHO  = 0o010000;
        const IMAXBEL = 0o020000;
        const ECHOKE  = 0o040000;
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct TermiosCFlag: u32 {
        const CSIZE  = 0x30;
        const CS5    = 0x0;
        const CS6    = 0x10;
        const CS7    = 0x20;
        const CS8    = 0x30;
        const CSTOPB = 0x40;
        const CREAD  = 0x80;
        const PARENB = 0x100;
        const PARODD = 0x200;
        const HUPCL  = 0x400;
        const CLOCAL = 0x800;
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct TermiosOFlag: u32 {
        const OPOST  = 0x1;
        const ONLCR  = 0x4;
        const OCRNL  = 0x8;
        const ONOCR  = 0x10;
        const ONLRET = 0x20;
        const OFDEL  = 0x80;
        const OFILL  = 0x40;
        const NLDLY  = 0x100;
        const NL0    = 0x0;
        const NL1    = 0x100;
        const CRDLY  = 0x600;
        const CR0    = 0x0;
        const CR1    = 0x200;
        const CR2    = 0x400;
        const CR3    = 0x600;
        const TABDLY = 0x1800;
        const TAB0   = 0x0;
        const TAB1   = 0x800;
        const TAB2   = 0x1000;
        const TAB3   = 0x1800;
        const XTABS  = 0x1800;
        const BSDLY  = 0x2000;
        const BS0    = 0x0;
        const BS1    = 0x2000;
        const VTDLY  = 0x4000;
        const VT0    = 0x0;
        const VT1    = 0x4000;
        const FFDLY  = 0x8000;
        const FF0    = 0x0;
        const FF1    = 0x8000;
    }
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct Termios {
    pub c_iflag: TermiosIFlag,
    pub c_oflag: TermiosOFlag,
    pub c_cflag: TermiosCFlag,
    pub c_lflag: TermiosLFlag,
    pub c_line: u8,
    pub c_cc: [u8; 32],
    pub c_ispeed: u32,
    pub c_ospeed: u32,
}

impl Termios {
    pub fn is_cooked(&self) -> bool {
        self.c_lflag.contains(TermiosLFlag::ICANON)
    }
}

pub const AT_FDCWD: isize = -100;

#[repr(C)]
#[derive(Debug)]
pub struct SysInfo {
    /// Seconds since boot
    pub uptime: i64,
    /// 1, 5, and 15 minute load averages
    pub loads: [u64; 3],
    /// Total usable main memory size.
    pub totalram: u64,
    /// Available memory size.
    pub freeram: u64,
    /// Amount of shared memory.
    pub sharedram: u64,
    /// Memory used by buffers.
    pub bufferram: u64,
    /// Total swap space size.
    pub totalswap: u64,
    /// Swap space still available.
    pub freeswap: u64,
    pub procs: u16,
    pub pad: u16,
    pub totalhigh: u64,
    pub freehigh: u64,
    pub mem_unit: u32,
    pub _f: [i8; 0],
}

pub fn syscall_result_as_usize(result: Result<usize>) -> usize {
    match result {
        Ok(value) => value as _,
        Err(error) => -(error as isize) as _,
    }
}

/// Inner helper function that converts the syscall result value into the
/// Rust [`Result`] type.
pub fn isize_as_syscall_result(value: isize) -> Result<usize> {
    if value >= 0 {
        Ok(value as usize)
    } else {
        let err: SyscallError = unsafe { core::mem::transmute((-value) as u64) };
        Err(err)
    }
}

pub fn sys_ipc_send(pid: usize, message: &[u8]) -> Result<()> {
    let value = syscall3(
        prelude::SYS_IPC_SEND,
        pid,
        message.as_ptr() as usize,
        message.len(),
    );
    isize_as_syscall_result(value as _).map(|_| ())
}

pub fn sys_ipc_recv<'a>(
    pid: &mut usize,
    message: &'a mut [u8],
    block: bool,
) -> Result<&'a mut [u8]> {
    let value = syscall4(
        prelude::SYS_IPC_RECV,
        pid as *mut usize as usize,
        message.as_ptr() as usize,
        message.len(),
        block as usize,
    );
    isize_as_syscall_result(value as _).map(|size| &mut message[0..size])
}

pub fn sys_ipc_discover_root() -> Result<usize> {
    let value = syscall0(prelude::SYS_IPC_DISCOVER_ROOT);
    isize_as_syscall_result(value as _)
}

pub fn sys_ipc_become_root() -> Result<()> {
    let value = syscall0(prelude::SYS_IPC_BECOME_ROOT);
    isize_as_syscall_result(value as _).map(|_| ())
}

// Sockets
pub trait SocketAddr: Send + Sync {}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SocketAddrUnix {
    pub family: u32,
    pub path: [u8; 108],
}

impl SocketAddrUnix {
    pub fn path_len(&self) -> u8 {
        if self.path[0] == 0 {
            if self.path[1] == 0 {
                // address is unnamed
                return 0;
            } else {
                // abstract socket address
                unimplemented!()
            }
        }

        (self.path.iter().position(|&c| c == 0).unwrap_or(108) as u8) + 1
    }
}

impl Default for SocketAddrUnix {
    fn default() -> Self {
        Self {
            family: AF_UNIX,
            path: [0; 108],
        }
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct InAddr {
    pub addr: u32,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SocketAddrInet {
    pub family: u32,
    pub port: BigEndian<u16>,
    pub sin_addr: InAddr,
    pub padding: [u8; 8],
}

impl SocketAddrInet {
    pub fn addr(&self) -> [u8; 4] {
        self.sin_addr.addr.to_le_bytes()
    }

    pub fn port(&self) -> u16 {
        self.port.to_native()
    }
}

impl SocketAddr for SocketAddrUnix {}
impl SocketAddr for SocketAddrInet {}

// mlibc/abi-bits/mlibc/in.h
#[derive(Debug, Copy, Clone, FromPrimitive, PartialEq)]
pub enum IpProtocol {
    Default = 0,
    Ip = 1,
    Ipv6 = 2,
    Icmp = 3,
    Raw = 4,
    Tcp = 5,
    Udp = 6,
    Igmp = 7,
    Ipip = 8,
    Dccp = 33,
    Routing = 43,
    Gre = 47,
    Esp = 50,
    Ah = 51,
    Icmpv6 = 58,
    Dstopts = 60,
    Comp = 108,
    Sctp = 132,
    Max = 256,
}

// mlibc/abi-bits/mlibc/socket.h
#[derive(Debug, Copy, Clone, FromPrimitive, PartialEq)]
pub enum SocketType {
    Dgram = 1,
    Raw = 2,
    SeqPacket = 3,
    Stream = 4,
    Dccp = 5,
}

bitflags::bitflags! {
    pub struct SocketFlags: usize {
        const NONBLOCK = 0x10000;
        const CLOEXEC  = 0x20000;
        const RDM      = 0x40000;
    }
}

impl From<SocketFlags> for OpenFlags {
    fn from(flags: SocketFlags) -> Self {
        let mut result = OpenFlags::empty();

        if flags.contains(SocketFlags::NONBLOCK) {
            result.insert(OpenFlags::O_NONBLOCK);
        }

        if flags.contains(SocketFlags::CLOEXEC) {
            result.insert(OpenFlags::O_CLOEXEC);
        }

        result
    }
}

pub const PF_INET: u32 = 1;
pub const PF_INET6: u32 = 2;
pub const PF_UNIX: u32 = 3;
pub const PF_LOCAL: u32 = 3;
pub const PF_UNSPEC: u32 = 4;
pub const PF_NETLINK: u32 = 5;
pub const PF_BRIDGE: u32 = 6;

pub const AF_INET: u32 = PF_INET;
pub const AF_INET6: u32 = PF_INET6;
pub const AF_UNIX: u32 = PF_UNIX;
pub const AF_LOCAL: u32 = PF_LOCAL;
pub const AF_UNSPEC: u32 = PF_UNSPEC;
pub const AF_NETLINK: u32 = PF_NETLINK;
pub const AF_BRIDGE: u32 = PF_BRIDGE;

// sysdeps/aero/include/abi-bits/stat.h
bitflags::bitflags! {
    #[derive(Default)]
    pub struct Mode: u32 {
        const S_IFMT   = 0x0F000;
        const S_IFBLK  = 0x06000;
        const S_IFCHR  = 0x02000;
        const S_IFIFO  = 0x01000;
        const S_IFREG  = 0x08000;
        const S_IFDIR  = 0x04000;
        const S_IFLNK  = 0x0A000;
        const S_IFSOCK = 0x0C000;

        const S_IRWXU = 0o700;
        const S_IRUSR = 0o400;
        const S_IWUSR = 0o200;
        const S_IXUSR = 0o100;
        const S_IRWXG = 0o70;
        const S_IRGRP = 0o40;
        const S_IWGRP = 0o20;
        const S_IXGRP = 0o10;
        const S_IRWXO = 0o7;
        const S_IROTH = 0o4;
        const S_IWOTH = 0o2;
        const S_IXOTH = 0o1;
        const S_ISUID = 0o4000;
        const S_ISGID = 0o2000;
        const S_ISVTX = 0o1000;

        const S_IREAD  = Self::S_IRUSR.bits();
        const S_IWRITE = Self::S_IWUSR.bits();
        const S_IEXEC  = Self::S_IXUSR.bits();
    }
}

// sysdeps/aero/include/abi-bits/stat.h
#[repr(C)]
#[derive(Debug, Default)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: Mode,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_atim: TimeSpec,
    pub st_mtim: TimeSpec,
    pub st_ctim: TimeSpec,
    pub st_blksize: u64,
    pub st_blocks: u64,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct AtFlags: usize {
        /// Allow empty relative pathname.
        const EMPTY_PATH = 1;
        /// Follow symbolic links.
        const SYMLINK_FOLLOW = 2;
        /// Do not follow symbolic links.
        const SYMLINK_NOFOLLOW = 4;
        /// Remove directory instead of unlinking file.
        const REMOVEDIR = 8;
        /// Test access permitted for effective IDs, not real IDs.
        const EACCESS = 512;
    }
}
