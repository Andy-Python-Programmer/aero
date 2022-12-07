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

use crate::OpenFlags;

// syscall number constants:
pub const SYS_READ: usize = 0;
pub const SYS_WRITE: usize = 1;
pub const SYS_OPEN: usize = 2;
pub const SYS_CLOSE: usize = 3;
pub const SYS_SHUTDOWN: usize = 4;
pub const SYS_EXIT: usize = 5;
pub const SYS_FORK: usize = 6;
pub const SYS_REBOOT: usize = 7;
pub const SYS_MMAP: usize = 8;
pub const SYS_MUNMAP: usize = 9;
pub const SYS_ARCH_PRCTL: usize = 10;
pub const SYS_GETDENTS: usize = 11;
pub const SYS_GETCWD: usize = 12;
pub const SYS_CHDIR: usize = 13;
pub const SYS_MKDIR: usize = 14;
pub const SYS_MKDIR_AT: usize = 15;
pub const SYS_RMDIR: usize = 16;
pub const SYS_EXEC: usize = 17;
pub const SYS_LOG: usize = 18;
pub const SYS_UNAME: usize = 19;
pub const SYS_WAITPID: usize = 20;
pub const SYS_IOCTL: usize = 21;
pub const SYS_GETPID: usize = 22;
pub const SYS_SOCKET: usize = 23;
pub const SYS_CONNECT: usize = 24;
pub const SYS_BIND: usize = 25;
pub const SYS_LISTEN: usize = 26;
pub const SYS_ACCEPT: usize = 27;
pub const SYS_SEEK: usize = 28;
pub const SYS_GETTID: usize = 29;
pub const SYS_GETTIME: usize = 30;
pub const SYS_SLEEP: usize = 31;
pub const SYS_ACCESS: usize = 32;
pub const SYS_PIPE: usize = 33;
pub const SYS_UNLINK: usize = 34;
pub const SYS_GETHOSTNAME: usize = 35;
pub const SYS_SETHOSTNAME: usize = 36;
pub const SYS_INFO: usize = 37;
pub const SYS_CLONE: usize = 38;
pub const SYS_SIGRETURN: usize = 39;
pub const SYS_SIGACTION: usize = 40;
pub const SYS_SIGPROCMASK: usize = 41;
pub const SYS_DUP: usize = 42;
pub const SYS_FCNTL: usize = 43;
pub const SYS_DUP2: usize = 44;
pub const SYS_IPC_SEND: usize = 45;
pub const SYS_IPC_RECV: usize = 46;
pub const SYS_IPC_DISCOVER_ROOT: usize = 47;
pub const SYS_IPC_BECOME_ROOT: usize = 48;
pub const SYS_STAT: usize = 49;
pub const SYS_FSTAT: usize = 50;
pub const SYS_READ_LINK: usize = 51;
pub const SYS_EPOLL_CREATE: usize = 52;
pub const SYS_EPOLL_PWAIT: usize = 53;
pub const SYS_EPOLL_CTL: usize = 54;
pub const SYS_EVENT_FD: usize = 55;
pub const SYS_KILL: usize = 56;
pub const SYS_FUTEX_WAIT: usize = 57;
pub const SYS_FUTEX_WAKE: usize = 58;
pub const SYS_LINK: usize = 59;
pub const SYS_BACKTRACE: usize = 60;
pub const SYS_POLL: usize = 61;
pub const SYS_EXIT_THREAD: usize = 62;
pub const SYS_SOCK_RECV: usize = 63;
pub const SYS_SETITIMER: usize = 64;
pub const SYS_GETITIMER: usize = 65;
pub const SYS_GETPPID: usize = 66;

// constants for fcntl()'s command argument:
pub const F_DUPFD: usize = 1;
pub const F_DUPFD_CLOEXEC: usize = 2;
pub const F_GETFD: usize = 3;
pub const F_SETFD: usize = 4;
pub const F_GETFL: usize = 5;
pub const F_SETFL: usize = 6;
pub const F_GETLK: usize = 7;
pub const F_SETLK: usize = 8;
pub const F_SETLKW: usize = 9;
pub const F_GETOWN: usize = 10;
pub const F_SETOWN: usize = 11;

// constants for fcntl()'s additional argument of F_GETFD and F_SETFD:
bitflags::bitflags! {
    pub struct FdFlags: usize {
        const CLOEXEC = 1;
    }
}

// constants for the epoll API:
bitflags::bitflags! {
    pub struct EPollFlags: usize {
        const CLOEXEC  = 1;
    }
}

pub const EPOLL_CTL_ADD: usize = 1;
pub const EPOLL_CTL_DEL: usize = 2;
pub const EPOLL_CTL_MOD: usize = 3;

// structures for the epoll API:
#[derive(Copy, Clone)]
#[repr(C)]
pub union EPollData {
    pub ptr: *mut u8,
    pub fd: i32,
    pub u32: u32,
    pub u64: u64,
}

// options/linux/include/sys/epoll.h
bitflags::bitflags! {
    pub struct EPollEventFlags: u32 {
        const IN        = 0x001;
        /// There is an exceptional condition on the file descriptor.
        const PRI       = 0x002;
        const OUT       = 0x004;
        const RDNORM    = 0x040;
        const RDBAND    = 0x080;
        const WRNORM    = 0x100;
        const WRBAND    = 0x200;
        const MSG       = 0x400;
        const ERR       = 0x008;
        const HUP       = 0x010;
        const RDHUP     = 0x2000;
        const EXCLUSIVE = 1 << 28;
        const WAKEUP    = 1 << 29;
        const ONESHOT   = 1 << 30;
        const ET        = 1 << 31;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct EPollEvent {
    pub events: EPollEventFlags,
    pub data: EPollData,
}

impl core::fmt::Debug for EPollEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EPollEvent")
            .field("events", &self.events)
            .finish()
    }
}

// structures for the poll API:
#[derive(Debug)]
pub struct PollFd {
    pub fd: i32,
    pub events: PollEventFlags,
    pub revents: PollEventFlags,
}

// sysdeps/aero/include/abi-bits/poll.h
bitflags::bitflags! {
    pub struct PollEventFlags: i16 {
        const IN     = 0x01;
        const OUT    = 0x02;
        const PRI    = 0x04;
        const HUP    = 0x08;
        const ERR    = 0x10;
        const RDHUP  = 0x20;
        const NVAL   = 0x40;
        const WRNORM = 0x80;
    }
}

// constants for generic ioctls (applicable to any file descriptor):
pub const FIONREAD: usize = 0x541b;
pub const FIONBIO: usize = 0x5421;
pub const FIONCLEX: usize = 0x5450;
pub const FIOCLEX: usize = 0x5451;

// constants for event fd:
bitflags::bitflags! {
    // mlibc/options/linux/include/sys/eventfd.h
    pub struct EventFdFlags: usize {
        const SEMAPHORE = 1;
        const CLOEXEC   = OpenFlags::O_CLOEXEC.bits();
        const NONBLOCK  = OpenFlags::O_NONBLOCK.bits();
    }
}

// framebuffer constants:
//
// NOTE: The framebuffer constants and structs are derived from the layout
// of these constants and structs in the linux usermode API.
//
// https://github.com/torvalds/linux/blob/master/include/uapi/linux/fb.h
//
// TODO: Move these constants and structures to the Aero UAPI crate.
pub const FBIOGET_VSCREENINFO: usize = 0x4600;
pub const FBIOPUT_VSCREENINFO: usize = 0x4601;
pub const FBIOGET_FSCREENINFO: usize = 0x4602;
pub const FBIOGETCMAP: usize = 0x4604;
pub const FBIOPUTCMAP: usize = 0x4605;

pub const FB_TYPE_PACKED_PIXELS: u32 = 0;
pub const FB_TYPE_PLANES: u32 = 1;
pub const FB_TYPE_INTERLEAVED_PLANES: u32 = 2;
pub const FB_TYPE_TEXT: u32 = 3;
pub const FB_TYPE_VGA_PLANES: u32 = 4;
pub const FB_TYPE_FOURCC: u32 = 5;

pub const FB_VISUAL_MONO01: u32 = 0;
pub const FB_VISUAL_MONO10: u32 = 1;
pub const FB_VISUAL_TRUECOLOR: u32 = 2;
pub const FB_VISUAL_PSEUDOCOLOR: u32 = 3;
pub const FB_VISUAL_DIRECTCOLOR: u32 = 4;
pub const FB_VISUAL_STATIC_PSEUDOCOLOR: u32 = 5;
pub const FB_VISUAL_FOURCC: u32 = 6;

pub const FB_ACTIVATE_NOW: u32 = 0;
pub const FB_ACTIVATE_NXTOPEN: u32 = 1;
pub const FB_ACTIVATE_TEST: u32 = 2;
pub const FB_ACTIVATE_MASK: u32 = 15;

pub const FB_VMODE_NONINTERLACED: u32 = 0;
pub const FB_VMODE_INTERLACED: u32 = 1;
pub const FB_VMODE_DOUBLE: u32 = 2;
pub const FB_VMODE_ODD_FLD_FIRST: u32 = 4;
pub const FB_VMODE_MASK: u32 = 255;

#[derive(Default, Debug, Clone)]
#[repr(C)]
pub struct FramebufferBitField {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

impl FramebufferBitField {
    pub fn new(shift: u32, size: u32) -> Self {
        Self {
            offset: shift,
            length: size,
            msb_right: 0,
        }
    }
}

// device independent colour information:
#[derive(Debug)]
#[repr(C)]
pub struct FramebufferCmap {
    pub start: u32, // first entry
    pub len: u32,   // number of entries
    pub red: *mut u16,
    pub green: *mut u16,
    pub blue: *mut u16,
    pub transp: *mut u16, // can be NULL
}

impl Default for FramebufferCmap {
    fn default() -> Self {
        Self {
            start: Default::default(),
            len: Default::default(),
            red: core::ptr::null_mut(),
            green: core::ptr::null_mut(),
            blue: core::ptr::null_mut(),
            transp: core::ptr::null_mut(),
        }
    }
}

// framebuffer variable screen info:
#[derive(Default, Debug, Clone)]
#[repr(C)]
pub struct FramebufferVScreenInfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,
    pub xoffset: u32,
    pub yoffset: u32,
    pub bits_per_pixel: u32,
    pub grayscale: u32,
    pub red: FramebufferBitField,
    pub green: FramebufferBitField,
    pub blue: FramebufferBitField,
    pub transp: FramebufferBitField,
    pub nonstd: u32,
    pub activate: u32,
    pub height: u32,
    pub width: u32,
    pub accel_flags: u32,
    pub pixclock: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub upper_margin: u32,
    pub lower_margin: u32,
    pub hsync_len: u32,
    pub vsync_len: u32,
    pub sync: u32,
    pub vmode: u32,
    pub rotate: u32,
    pub colorspace: u32,
    pub reserved: [u32; 4],
}

// framebuffer fixed screen info:
#[derive(Default, Debug, Clone)]
#[repr(C)]
pub struct FramebufferFScreenInfo {
    pub id: [u8; 16],
    pub smem_start: u64,
    pub smem_len: u32,
    pub typee: u32,
    pub type_aux: u32,
    pub visual: u32,
    pub xpanstep: u16,
    pub ypanstep: u16,
    pub ywrapstep: u16,
    pub line_length: u32,
    pub mmio_start: u64,
    pub mmio_len: u32,
    pub accel: u32,
    pub capabilities: u16,
    pub reserved: [u16; 2],
}
