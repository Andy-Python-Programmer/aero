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

// fcntl constants:
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
