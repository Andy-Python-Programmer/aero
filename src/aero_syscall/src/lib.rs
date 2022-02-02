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

#![no_std]
#![feature(decl_macro)]

pub mod consts;
pub mod signal;
pub mod syscall;

pub use crate::syscall::*;

pub mod prelude {
    pub use crate::consts::*;
    pub use crate::syscall::*;

    pub use crate::AeroSyscallError;
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
        const O_ACCMODE   = 0x0007;
        const O_EXEC      = 1;
        const O_RDONLY    = 2;
        const O_RDWR      = 3;
        const O_SEARCH    = 4;
        const O_WRONLY    = 5;
        const O_APPEND    = 0x0008;
        const O_CREAT     = 0x0010;
        const O_DIRECTORY =  0x0020;
        const O_EXCL      =  0x0040;
        const O_NOCTTY    = 0x0080;
        const O_NOFOLLOW  = 0x0100;
        const O_TRUNC     = 0x0200;
        const O_NONBLOCK  = 0x0400;
        const O_DSYNC     = 0x0800;
        const O_RSYNC     = 0x1000;
        const O_SYNC      = 0x2000;
        const O_CLOEXEC   = 0x4000;
        const O_PATH      = 0x8000;
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(isize)]
pub enum AeroSyscallError {
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

pub fn syscall_as_str(syscall: usize) -> &'static str {
    match syscall {
        prelude::SYS_READ => "read",
        prelude::SYS_WRITE => "write",
        prelude::SYS_OPEN => "open",
        prelude::SYS_CLOSE => "close",
        prelude::SYS_SHUTDOWN => "shutdown",
        prelude::SYS_EXIT => "exit",
        prelude::SYS_FORK => "fork",
        prelude::SYS_REBOOT => "reboot",
        prelude::SYS_MMAP => "mmap",
        prelude::SYS_MUNMAP => "munmap",
        prelude::SYS_ARCH_PRCTL => "arch_prctl",
        prelude::SYS_GETDENTS => "getdents",
        prelude::SYS_GETCWD => "getcwd",
        prelude::SYS_CHDIR => "chdir",
        prelude::SYS_MKDIR => "mkdir",
        prelude::SYS_MKDIR_AT => "mkdir_at",
        prelude::SYS_RMDIR => "rmdir",
        prelude::SYS_EXEC => "exec",
        prelude::SYS_LOG => "log",
        prelude::SYS_UNAME => "uname",
        prelude::SYS_WAITPID => "waitpid",
        prelude::SYS_IOCTL => "ioctl",
        prelude::SYS_GETPID => "getpid",
        prelude::SYS_SOCKET => "socket",
        prelude::SYS_CONNECT => "connect",
        prelude::SYS_BIND => "bind",
        prelude::SYS_LISTEN => "listen",
        prelude::SYS_ACCEPT => "accept",
        prelude::SYS_SEEK => "seek",
        prelude::SYS_GETTID => "gettid",
        prelude::SYS_GETTIME => "gettime",
        prelude::SYS_SLEEP => "sleep",
        prelude::SYS_ACCESS => "access",
        prelude::SYS_PIPE => "pipe",
        prelude::SYS_UNLINK => "unlink",
        prelude::SYS_GETHOSTNAME => "gethostname",
        prelude::SYS_SETHOSTNAME => "sethostname",
        prelude::SYS_INFO => "info",
        prelude::SYS_CLONE => "clone",
        prelude::SYS_SIGRETURN => "sigreturn",
        prelude::SYS_SIGACTION => "sigaction",
        prelude::SYS_SIGPROCMASK => "sigprocmask",
        prelude::SYS_DUP => "dup",
        prelude::SYS_FCNTL => "fcntl",
        prelude::SYS_DUP2 => "dup2",

        _ => unreachable!("unknown syscall"),
    }
}

#[derive(Debug)]
#[repr(usize)]
pub enum SysFileType {
    File,
    Directory,
    Device,
    Socket,
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
pub struct Utsname {
    pub name: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
}

impl Utsname {
    pub fn name(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.name) }
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
            name: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
        }
    }
}

#[derive(Default, Clone)]
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
pub const TCGETS: usize = 0x5401;
pub const TCSETSF: usize = 0x5404;

#[derive(Default)]
#[repr(C)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct TermiosLFlag: u32 {
        const ECHO   =  0x0001;
        const ECHOE  =  0x0002;
        const ECHOK  =  0x0004;
        const ECHONL =  0x0008;
        const ICANON =  0x0010;
        const IEXTEN =  0x0020;
        const ISIG   =  0x0040;
        const NOFLSH =  0x0080;
        const TOSTOP =  0x0100;
        const ECHOPRT=  0x0200;
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct TermiosCFlag: u32 {
        const CSIZE  =  0x0003;
        const CS5    =  0x0000;
        const CS6    =  0x0001;
        const CS7    =  0x0002;
        const CS8    =  0x0003;
        const CSTOPB =  0x0004;
        const CREAD  =  0x0008;
        const PARENB =  0x0010;
        const PARODD =  0x0020;
        const HUPCL  =  0x0040;
        const CLOCAL =  0x0080;
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    pub struct TermiosOFlag: u32 {
        const OPOST  =  0x0001;
        const ONLCR  =  0x0002;
        const OCRNL  =  0x0004;
        const ONOCR  =  0x0008;
        const ONLRET =  0x0010;
        const OFDEL  =  0x0020;
        const OFILL  =  0x0040;
        const NLDLY  =  0x0080;
        const NL0    =  0x0000;
        const NL1    =  0x0080;
        const CRDLY  =  0x0300;
        const CR0    =  0x0000;
        const CR1    =  0x0100;
        const CR2    =  0x0200;
        const CR3    =  0x0300;
        const TABDLY =  0x0C00;
        const TAB0   =  0x0000;
        const TAB1   =  0x0400;
        const TAB2   =  0x0800;
        const TAB3   =  0x0C00;
        const BSDLY  =  0x1000;
        const BS0    =  0x0000;
        const BS1    =  0x1000;
        const VTDLY  =  0x2000;
        const VT0    =  0x0000;
        const VT1    =  0x2000;
        const FFDLY  =  0x4000;
        const FF0    =  0x0000;
        const FF1    =  0x4000;
    }
}

#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: TermiosOFlag,
    pub c_cflag: TermiosCFlag,
    pub c_lflag: TermiosLFlag,
    pub c_line: u8,
    pub c_cc: [u8; 11],
    pub c_ispeed: u32,
    pub c_ospeed: u32,
}

pub const AT_FDCWD: isize = -100;

#[repr(C)]
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

pub fn syscall_result_as_usize(result: Result<usize, AeroSyscallError>) -> usize {
    match result {
        Ok(value) => value as _,
        Err(error) => -(error as isize) as _,
    }
}

/// Inner helper function that converts the syscall result value into the
/// Rust [`Result`] type.
fn isize_as_syscall_result(value: isize) -> Result<usize, AeroSyscallError> {
    if value >= 0 {
        Ok(value as usize)
    } else {
        let err: AeroSyscallError = unsafe { core::mem::transmute((-value) as u64) };
        Err(err)
    }
}

pub fn sys_exit(status: usize) -> ! {
    syscall1(prelude::SYS_EXIT, status);
    unreachable!()
}

pub fn sys_open(path: &str, mode: OpenFlags) -> Result<usize, AeroSyscallError> {
    let value = syscall4(
        prelude::SYS_OPEN,
        0x00,
        path.as_ptr() as usize,
        path.len(),
        mode.bits(),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_write(fd: usize, buf: &[u8]) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_WRITE,
        fd as usize,
        buf.as_ptr() as usize,
        buf.len(),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_read(fd: usize, buf: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_READ,
        fd as usize,
        buf.as_mut_ptr() as usize,
        buf.len(),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_chdir(path: &str) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_CHDIR, path.as_ptr() as usize, path.len());
    isize_as_syscall_result(value as _)
}

pub fn sys_close(fd: usize) -> Result<usize, AeroSyscallError> {
    let value = syscall1(prelude::SYS_CLOSE, fd);
    isize_as_syscall_result(value as _)
}

pub fn sys_getcwd(buf: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_GETCWD, buf.as_mut_ptr() as usize, buf.len());
    isize_as_syscall_result(value as _)
}

pub fn sys_getdents(fd: usize, buf: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_GETDENTS,
        fd as usize,
        buf.as_mut_ptr() as usize,
        buf.len(),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_fork() -> Result<usize, AeroSyscallError> {
    let value = syscall0(prelude::SYS_FORK);
    isize_as_syscall_result(value as _)
}

pub fn sys_munmap(address: usize, size: usize) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_MUNMAP, address as usize, size as usize);
    isize_as_syscall_result(value as _)
}

pub fn sys_mkdir(path: &str) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_MKDIR, path.as_ptr() as usize, path.len());
    isize_as_syscall_result(value as _)
}

pub fn sys_log(message: &str) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_LOG, message.as_ptr() as usize, message.len());
    isize_as_syscall_result(value as _)
}

pub fn sys_mkdirat(dfd: isize, path: &str) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_MKDIR_AT,
        dfd as usize,
        path.as_ptr() as usize,
        path.len(),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_exec(path: &str, argv: &[&str], envv: &[&str]) -> Result<usize, AeroSyscallError> {
    let value = syscall6(
        prelude::SYS_EXEC,
        path.as_ptr() as usize,
        path.len(),
        argv.as_ptr() as usize,
        argv.len(),
        envv.as_ptr() as usize,
        envv.len(),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_rmdir(path: &str) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_RMDIR, path.as_ptr() as usize, path.len());
    isize_as_syscall_result(value as _)
}

pub fn sys_uname(struc: &mut Utsname) -> Result<usize, AeroSyscallError> {
    let value = syscall1(prelude::SYS_UNAME, struc as *mut Utsname as usize);
    isize_as_syscall_result(value as _)
}

pub fn sys_shutdown() -> ! {
    syscall0(prelude::SYS_SHUTDOWN);
    unreachable!()
}

pub fn sys_access(fd: usize, path: &str) -> Result<usize, AeroSyscallError> {
    let value = syscall5(
        prelude::SYS_ACCESS,
        fd,
        path.as_ptr() as usize,
        path.len(),
        0,
        0,
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_waitpid(pid: usize, status: &mut u32, flags: usize) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_WAITPID,
        pid as usize,
        status as *mut u32 as usize,
        flags,
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_ioctl(fd: usize, command: usize, arg: usize) -> Result<usize, AeroSyscallError> {
    let value = syscall3(prelude::SYS_IOCTL, fd as usize, command, arg);
    isize_as_syscall_result(value as _)
}

pub fn sys_mmap(
    address: usize,
    size: usize,
    protection: MMapProt,
    flags: MMapFlags,
    fd: usize,
    offset: usize,
) -> Result<usize, AeroSyscallError> {
    let value = syscall6(
        prelude::SYS_MMAP,
        address,
        size,
        protection.bits(),
        flags.bits(),
        fd,
        offset,
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_getpid() -> Result<usize, AeroSyscallError> {
    let value = syscall0(prelude::SYS_GETPID);
    isize_as_syscall_result(value as _)
}

pub fn sys_gettid() -> Result<usize, AeroSyscallError> {
    let value = syscall0(prelude::SYS_GETTID);
    isize_as_syscall_result(value as _)
}

pub fn sys_gethostname(buf: &mut [u8]) -> Result<usize, AeroSyscallError> {
    let value = syscall2(
        prelude::SYS_GETHOSTNAME,
        buf.as_mut_ptr() as usize,
        buf.len(),
    );
    isize_as_syscall_result(value as _)
}

pub fn sys_sethostname(name: &str) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_SETHOSTNAME, name.as_ptr() as usize, name.len());
    isize_as_syscall_result(value as _)
}

// Sockets
#[derive(Debug, Clone)]
#[repr(C)]
pub struct SocketAddrUnix {
    pub family: i16,
    pub path: [u8; 108],
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct SocketAddrInet {
    pub family: i16,
    pub port: [u8; 2],
    pub address: [u8; 4],
    pub padding: [u8; 8],
}

#[derive(Debug, Clone)]
pub enum SocketAddr {
    Unix(SocketAddrUnix),
    Inet(SocketAddrInet),
}

pub const AF_UNIX: usize = 1;
pub const AF_INET: usize = 2;

pub const SOCK_STREAM: usize = 1;
pub const SOCK_DGRAM: usize = 2;

pub const IPPROTO_TCP: usize = 6;
pub const IPPROTO_UDP: usize = 17;

pub fn sys_socket(
    domain: usize,
    socket_type: usize,
    protocol: usize,
) -> Result<usize, AeroSyscallError> {
    let value = syscall3(prelude::SYS_SOCKET, domain, socket_type, protocol);
    isize_as_syscall_result(value as _)
}

pub fn sys_connect(
    fd: usize,
    address: &SocketAddr,
    length: u32,
) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_CONNECT,
        fd,
        address as *const SocketAddr as usize,
        length as usize,
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_bind(fd: usize, address: &SocketAddr, length: u32) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_BIND,
        fd,
        address as *const SocketAddr as usize,
        length as usize,
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_listen(fd: usize, backlog: usize) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_LISTEN, fd, backlog);
    isize_as_syscall_result(value as _)
}

pub fn sys_accept(
    fd: usize,
    address: &mut SocketAddr,
    length: &mut u32,
) -> Result<usize, AeroSyscallError> {
    let value = syscall3(
        prelude::SYS_ACCEPT,
        fd,
        address as *const SocketAddr as usize,
        length as *mut u32 as usize,
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_unlink(fd: usize, path: &str, flags: OpenFlags) -> Result<usize, AeroSyscallError> {
    let value = syscall4(
        prelude::SYS_UNLINK,
        fd,
        path.as_ptr() as usize,
        path.len(),
        flags.bits(),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_gettime(clock: usize, timespec: &mut TimeSpec) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_GETTIME, clock, timespec as *mut _ as usize);
    isize_as_syscall_result(value as _)
}

pub fn sys_seek(fd: usize, offset: usize, whence: SeekWhence) -> Result<usize, AeroSyscallError> {
    let value = syscall3(prelude::SYS_SEEK, fd, offset, whence as usize);
    isize_as_syscall_result(value as _)
}

pub fn sys_sleep(timespec: &TimeSpec) -> Result<usize, AeroSyscallError> {
    let value = syscall1(prelude::SYS_SLEEP, timespec as *const _ as usize);
    isize_as_syscall_result(value as _)
}

pub fn sys_pipe(fds: &mut [usize; 2], flags: OpenFlags) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_PIPE, fds.as_ptr() as usize, flags.bits());
    isize_as_syscall_result(value as _)
}

pub fn sys_info(struc: &mut SysInfo) -> Result<usize, AeroSyscallError> {
    let value = syscall1(prelude::SYS_INFO, struc as *mut _ as usize);
    isize_as_syscall_result(value as _)
}

pub fn sys_clone(entry: usize, stack: usize) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_CLONE, entry, stack);
    isize_as_syscall_result(value as _)
}

pub fn sys_sigreturn() -> Result<usize, AeroSyscallError> {
    let value = syscall0(prelude::SYS_SIGRETURN);
    isize_as_syscall_result(value as _)
}

pub fn sys_sigaction(
    sig: usize,
    sigaction: Option<&signal::SigAction>,
    old_sigaction: Option<&mut signal::SigAction>,
) -> Result<usize, AeroSyscallError> {
    let sigact = sigaction;

    let value = syscall4(
        prelude::SYS_SIGACTION,
        sig,
        sigact
            .and_then(|f| Some(f as *const signal::SigAction as usize))
            .unwrap_or(0),
        sys_sigreturn as usize,
        old_sigaction
            .and_then(|f| Some(f as *mut signal::SigAction as usize))
            .unwrap_or(0),
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_sigprocmask(
    how: signal::SigProcMask,
    set: &mut u64,
    old_set: Option<&mut u64>,
) -> Result<usize, AeroSyscallError> {
    let old_set = match old_set {
        Some(e) => e as *const u64 as usize,
        None => 0,
    };

    let value = syscall3(
        prelude::SYS_SIGPROCMASK,
        how as usize,
        set as *const u64 as usize,
        old_set,
    );

    isize_as_syscall_result(value as _)
}

pub fn sys_dup(fd: usize, flags: OpenFlags) -> Result<usize, AeroSyscallError> {
    let value = syscall2(prelude::SYS_DUP, fd, flags.bits());
    isize_as_syscall_result(value as _)
}

pub fn sys_fcntl(fd: usize, command: usize, argument: usize) -> Result<usize, AeroSyscallError> {
    let value = syscall3(prelude::SYS_FCNTL, fd, command, argument);
    isize_as_syscall_result(value as _)
}

pub fn sys_dup2(fd: usize, new_fd: usize, flags: OpenFlags) -> Result<usize, AeroSyscallError> {
    let value = syscall3(prelude::SYS_DUP2, fd, new_fd, flags.bits());
    isize_as_syscall_result(value as _)
}
