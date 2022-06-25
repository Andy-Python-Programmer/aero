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

// structures and uninons for the epoll API:
#[derive(Copy, Clone)]
#[repr(C)]
pub union EPollData {
    pub ptr: *mut u8,
    pub fd: i32,
    pub u32: u32,
    pub u64: u64,
}

bitflags::bitflags! {
    /// A bit mask composed by ORing together, zero or more of the following available event types.
    #[derive(Default)]
    pub struct EPollEventFlags: u32 {
        /// The associated file is available for read operations.
        const IN        = 0x001;
        /// There is an exceptional condition on the file descriptor.
        const PRI       = 0x002;
        /// The associated file is available for write operations.
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
pub const FBIOGET_VSCREENINFO: usize = 0x4600;

#[derive(Default, Clone)]
pub struct FramebufferBitField {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

// framebuffer variable screen info:
#[derive(Default, Clone)]
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
