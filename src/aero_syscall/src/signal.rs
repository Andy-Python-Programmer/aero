pub const SIGHUP: usize = 1;
pub const SIGINT: usize = 2;
pub const SIGQUIT: usize = 3;
pub const SIGILL: usize = 4;
pub const SIGBUS: usize = 7;
pub const SIGFPE: usize = 8;
pub const SIGKILL: usize = 9;
pub const SIGSEGV: usize = 11;
pub const SIGPIPE: usize = 13;
pub const SIGTERM: usize = 15;
pub const SIGCHLD: usize = 17;
pub const SIGCONT: usize = 18;
pub const SIGSTOP: usize = 19;
pub const SIGTSTP: usize = 20;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SignalHandler {
    Ignore,
    Default,
    Handle(fn(usize)),
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
    #[derive(Default)]
    pub struct SignalFlags: u32 {
        const SA_NOCLDSTOP = (1 << 0);
        const SA_ONSTACK   = (1 << 1);
        const SA_RESETHAND = (1 << 2);
        const SA_RESTART   = (1 << 3);
        const SA_SIGINFO   = (1 << 4);
        const SA_NOCLDWAIT = (1 << 5);
        const SA_NODEFER   = (1 << 6);
    }
}

#[repr(u64)]
#[derive(Debug)]
pub enum SigProcMask {
    Block = 1,
    Unblock = 2,
    Set = 3,
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
            -3 => SignalHandler::Ignore,
            -2 => SignalHandler::Default,
            v => SignalHandler::Handle(unsafe { core::mem::transmute::<u64, fn(usize)>(v as u64) }),
        }
    }
}

impl From<SignalHandler> for usize {
    fn from(h: SignalHandler) -> Self {
        match h {
            SignalHandler::Ignore => -3isize as usize,
            SignalHandler::Default => -2isize as usize,
            SignalHandler::Handle(f) => f as usize,
        }
    }
}

impl From<SignalHandler> for u64 {
    fn from(h: SignalHandler) -> Self {
        match h {
            SignalHandler::Ignore => -3isize as u64,
            SignalHandler::Default => -2isize as u64,
            SignalHandler::Handle(f) => f as u64,
        }
    }
}

impl From<u64> for SigProcMask {
    fn from(v: u64) -> Self {
        match v {
            1 => SigProcMask::Block,
            2 => SigProcMask::Unblock,
            3 => SigProcMask::Set,
            _ => panic!("invalid signal procmask {}", v),
        }
    }
}

impl From<SigProcMask> for usize {
    fn from(s: SigProcMask) -> Self {
        s as u64 as usize
    }
}
