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

// mlibc/abis/linux/signal.h
pub const SIGABRT: usize = 6;
pub const SIGFPE: usize = 8;
pub const SIGILL: usize = 4;
pub const SIGINT: usize = 2;
pub const SIGSEGV: usize = 11;
pub const SIGTERM: usize = 15;
pub const SIGPROF: usize = 27;
pub const SIGIO: usize = 29;
pub const SIGPWR: usize = 30;
pub const SIGRTMIN: usize = 35;
pub const SIGRTMAX: usize = 64;

// constants for sigprocmask()
pub const SIG_BLOCK: u64 = 0;
pub const SIG_UNBLOCK: u64 = 1;
pub const SIG_SETMASK: u64 = 2;

pub const SIGHUP: usize = 1;
pub const SIGQUIT: usize = 3;
pub const SIGTRAP: usize = 5;
pub const SIGIOT: usize = SIGABRT;
pub const SIGBUS: usize = 7;
pub const SIGKILL: usize = 9;
pub const SIGUSR1: usize = 10;
pub const SIGUSR2: usize = 12;
pub const SIGPIPE: usize = 13;
pub const SIGALRM: usize = 14;
pub const SIGSTKFLT: usize = 16;
pub const SIGCHLD: usize = 17;
pub const SIGCONT: usize = 18;
pub const SIGSTOP: usize = 19;
pub const SIGTSTP: usize = 20;
pub const SIGTTIN: usize = 21;
pub const SIGTTOU: usize = 22;
pub const SIGURG: usize = 23;
pub const SIGXCPU: usize = 24;
pub const SIGXFSZ: usize = 25;
pub const SIGVTALRM: usize = 26;
pub const SIGWINCH: usize = 28;
pub const SIGPOLL: usize = 29;
pub const SIGSYS: usize = 31;
pub const SIGUNUSED: usize = SIGSYS;
pub const SIGCANCEL: usize = 32;

pub const SIG_ERR: i64 = -1; // error
pub const SIG_DFL: i64 = 0; // default
pub const SIG_IGN: i64 = 1; // ignore

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SignalHandler {
    Ignore,
    Default,
    Handle(fn(usize)),
}

impl Default for SignalHandler {
    fn default() -> Self {
        SignalHandler::Default
    }
}

impl From<u64> for SignalHandler {
    fn from(v: u64) -> Self {
        let v = v as i64;
        match v {
            SIG_IGN => SignalHandler::Ignore,
            SIG_DFL => SignalHandler::Default,
            v => SignalHandler::Handle(unsafe { core::mem::transmute::<u64, fn(usize)>(v as u64) }),
        }
    }
}

impl From<SignalHandler> for usize {
    fn from(h: SignalHandler) -> Self {
        match h {
            SignalHandler::Ignore => SIG_IGN as usize,
            SignalHandler::Default => SIG_DFL as usize,
            SignalHandler::Handle(f) => f as usize,
        }
    }
}

impl From<SignalHandler> for u64 {
    fn from(h: SignalHandler) -> Self {
        match h {
            SignalHandler::Ignore => SIG_IGN as u64,
            SignalHandler::Default => SIG_DFL as u64,
            SignalHandler::Handle(f) => f as u64,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SigAction {
    pub sa_handler: u64,
    pub sa_mask: u64,
    pub sa_flags: u32,
    pub sa_sigaction: u64,
}

impl SigAction {
    pub fn new(handler: SignalHandler, mask: u64, flags: SignalFlags) -> SigAction {
        SigAction {
            sa_handler: handler.into(),
            sa_mask: mask,
            sa_flags: flags.bits(),
            sa_sigaction: 0,
        }
    }
}

bitflags::bitflags! {
    // mlibc/abis/linux/signal.h
    #[derive(Default)]
    pub struct SignalFlags: u32 {
        const SA_NOCLDSTOP = 1;
        const SA_NOCLDWAIT = 2;
        const SA_SIGINFO = 4;
        const SA_ONSTACK = 0x08000000;
        const SA_RESTART = 0x10000000;
        const SA_NODEFER = 0x40000000;
        const SA_RESETHAND = 0x80000000;
        const SA_RESTORER = 0x04000000;
    }
}

#[repr(u64)]
#[derive(Debug)]
pub enum SigProcMask {
    Block = SIG_BLOCK,
    Unblock = SIG_UNBLOCK,
    Set = SIG_SETMASK,
}

impl From<u64> for SigProcMask {
    fn from(v: u64) -> Self {
        match v {
            SIG_BLOCK => SigProcMask::Block,
            SIG_UNBLOCK => SigProcMask::Unblock,
            SIG_SETMASK => SigProcMask::Set,
            _ => panic!("invalid signal procmask {}", v),
        }
    }
}

impl From<SigProcMask> for usize {
    fn from(s: SigProcMask) -> Self {
        s as u64 as usize
    }
}
