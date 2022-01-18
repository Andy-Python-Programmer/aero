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
