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

//! System Calls are used to call a kernel service from userland.
//!
//! | %rax   | Name                    |
//! |--------|-------------------------|
//! | 0      | read                    |
//! | 1      | write                   |
//! | 2      | open                    |
//! | 3      | close                   |
//! | 4      | shutdown                |
//! | 5      | exit                    |
//! | 6      | fork                    |
//! | 7      | reboot                  |
//! | 8      | mmap                    |
//! | 9      | munmap                  |
//! | 10     | arch_prctl              |
//! | 11     | get_dents               |
//! | 12     | get_cwd                 |
//! | 13     | chdir                   |
//! | 14     | mkdir                   |
//! | 15     | mkdirat                 |
//! | 16     | rmdir                   |
//! | 17     | exec                    |
//! | 18     | log                     |
//! | 19     | uname                   |
//! | 20     | waitpid                 |
//! | 21     | ioctl                   |
//! | 22     | getpid                  |
//! | 23     | socket                  |
//! | 24     | connect                 |
//! | 25     | bind                    |
//! | 26     | listen                  |
//! | 27     | accept                  |
//! | 28     | seek                    |
//! | 29     | gettid                  |
//! | 30     | gettime                 |
//! | 31     | sleep                   |
//! | 32     | access                  |
//! | 33     | pipe                    |
//! | 34     | unlink                  |
//! | 35     | gethostname             |
//! | 36     | sethostname             |
//! | 37     | info                    |
//! | 38     | clone                   |
//! | 39     | sigreturn               |
//! | 40     | sigaction               |
//! | 41     | sigprocmask             |
//! | 42     | dup                     |
//! | 43     | fcntl                   |
//! | 44     | dup2                    |
//! | 45     | ipc_send                |
//! | 46     | ipc_recv                |
//! | 47     | ipc_discover_root       |
//! | 48     | ipc_become_root         |
//! | 49     | stat                    |
//! | 50     | fstat                   |
//! | 51     | read_link               |

use core::mem::MaybeUninit;

use aero_syscall::prelude::*;

mod fs;
mod futex;
mod ipc;
mod net;
mod process;
mod time;

use alloc::boxed::Box;
use alloc::vec::Vec;

pub use fs::*;
pub use ipc::*;
pub use process::*;
pub use time::*;

use crate::utils::StackHelper;

#[derive(Default)]
pub struct ExecArgs {
    pub inner: Vec<Box<[u8]>>,
}

impl ExecArgs {
    pub fn push(&mut self, arg: &[u8]) {
        self.inner.push(arg.into());
    }

    pub fn extend(&mut self, args: &[Box<[u8]>]) {
        for arg in args {
            self.push(arg);
        }
    }

    pub fn push_into_stack(&self, stack: &mut StackHelper) -> Vec<u64> {
        let mut tops = Vec::with_capacity(self.inner.len());

        for slice in self.inner.iter() {
            unsafe {
                stack.write(0u8);
                stack.write_bytes(slice);
            }

            tops.push(stack.top());
        }

        tops
    }
}

pub fn exec_args_from_slice(args: usize, size: usize) -> ExecArgs {
    // NOTE: Arguments must be moved into kernel space before we utilize them.
    //
    // struct SliceReference {
    //    ptr: *const usize,
    //    len: usize,
    // }
    let data = args as *const [usize; 2];
    let slice = unsafe { core::slice::from_raw_parts(data, size) };

    // todo(andy): use `with_capacity` to avoid reallocation.
    let mut result = Vec::new();

    for inner in slice {
        let mut boxed = Box::new_uninit_slice(inner[1]);
        let ptr = inner[0] as *const MaybeUninit<u8>;

        unsafe {
            boxed.as_mut_ptr().copy_from(ptr, inner[1]);

            let inner_slice = boxed.assume_init();
            result.push(inner_slice);
        }
    }

    ExecArgs { inner: result }
}

#[cfg(feature = "syslog")]
pub(super) struct SysLog {
    name: &'static str,
    /// The result of the syscall.
    result: Option<Result<usize, SyscallError>>,
    /// The formatted argument values.
    args: Vec<String>,
}

#[cfg(feature = "syslog")]
impl SysLog {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            result: None,
            args: Vec::new(),
        }
    }

    pub fn add_argument<T>(mut self, value: T) -> Self
    where
        T: core::fmt::Display,
    {
        self.args.push(alloc::format!("{value}"));
        self
    }

    pub fn add_argument_dbg<T>(mut self, value: T) -> Self
    where
        T: core::fmt::Debug,
    {
        self.args.push(alloc::format!("{value:?}"));
        self
    }

    pub fn set_result(mut self, result: Result<usize, SyscallError>) -> Self {
        self.result = Some(result);
        self
    }

    pub fn flush(self) {
        let mut result = String::new();

        if self.result.unwrap().is_ok() {
            result.push_str("\x1b[1;32m");
        } else {
            result.push_str("\x1b[1;31m");
        }

        result.push_str(alloc::format!("{}\x1b[0m(", self.name).as_str());

        for (i, e) in self.args.iter().enumerate() {
            if i != 0 {
                result.push_str(", ");
            }

            result.push_str(e);
        }

        result.push_str(alloc::format!(") = {:?}", self.result.unwrap()).as_str());
        log::trace!("{result}");
    }
}

pub fn generic_do_syscall(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    e: usize,
    f: usize,
    g: usize,
) -> usize {
    let result = match a {
        SYS_EXIT => process::exit(b),
        SYS_SHUTDOWN => process::shutdown(),
        SYS_FORK => process::fork(),
        SYS_MMAP => process::mmap(b, c, d, e, f, g),
        SYS_MUNMAP => process::munmap(b, c),
        SYS_EXEC => process::exec(b, c, d, e, f, g),
        SYS_LOG => process::log(b, c),
        SYS_UNAME => process::uname(b),
        SYS_WAITPID => process::waitpid(b, c, d),
        SYS_GETPID => process::getpid(),
        SYS_GETTID => process::gettid(),
        SYS_GETHOSTNAME => process::gethostname(b, c),
        SYS_SETHOSTNAME => process::sethostname(b, c),
        SYS_INFO => process::info(b),
        SYS_SIGACTION => process::sigaction(b, c, d, e),
        SYS_SIGPROCMASK => process::sigprocmask(b, c, d),
        SYS_CLONE => process::clone(b, c),
        SYS_KILL => process::kill(b, c),
        SYS_BACKTRACE => process::backtrace(),

        SYS_READ => fs::read(b, c, d),
        SYS_OPEN => fs::open(b, c, d, e),
        SYS_CLOSE => fs::close(b),
        SYS_WRITE => fs::write(b, c, d),
        SYS_GETDENTS => fs::getdents(b, c, d),
        SYS_GETCWD => fs::getcwd(b, c),
        SYS_CHDIR => fs::chdir(b, c),
        SYS_MKDIR_AT => fs::mkdirat(b, c, d),
        SYS_RMDIR => fs::rmdir(b, c),
        SYS_IOCTL => fs::ioctl(b, c, d),
        SYS_SEEK => fs::seek(b, c, d),
        SYS_ACCESS => fs::access(b, c, d, e, f),
        SYS_PIPE => fs::pipe(b, c),
        SYS_UNLINK => fs::unlink(b, c, d, e),
        SYS_DUP => fs::dup(b, c),
        SYS_DUP2 => fs::dup2(b, c, d),
        SYS_FCNTL => fs::fcntl(b, c, d),
        SYS_STAT => fs::stat(b, c, d),
        SYS_FSTAT => fs::fstat(b, c),
        SYS_READ_LINK => fs::read_link(b, c, d, e),
        SYS_EVENT_FD => fs::event_fd(b, c),
        SYS_LINK => fs::link(b, c, d, e),
        SYS_POLL => fs::poll(b, c, d, e),

        // epoll calls:
        SYS_EPOLL_CREATE => fs::epoll_create(b),
        SYS_EPOLL_CTL => fs::epoll_ctl(b, c, d, e),
        SYS_EPOLL_PWAIT => fs::epoll_pwait(b, c, d, e, f),

        SYS_SOCKET => net::socket(b, c, d),
        SYS_BIND => net::bind(b, c, d),
        SYS_CONNECT => net::connect(b, c, d),
        SYS_LISTEN => net::listen(b, c),
        SYS_ACCEPT => net::accept(b, c, d),
        SYS_SOCK_RECV => net::sock_recv(b, c, d),

        SYS_GETTIME => time::gettime(b, c),
        SYS_SLEEP => time::sleep(b),

        SYS_SETITIMER => time::setitimer(b, c, d),
        SYS_GETITIMER => time::getitimer(b, c),

        SYS_IPC_SEND => ipc::send(b, c, d),
        SYS_IPC_RECV => ipc::recv(b, c, d, e),
        SYS_IPC_DISCOVER_ROOT => ipc::discover_root(),
        SYS_IPC_BECOME_ROOT => ipc::become_root(),

        SYS_FUTEX_WAIT => futex::wait(b, c, d),
        SYS_FUTEX_WAKE => futex::wake(b),

        // Syscall aliases (this should be handled in aero_syscall)
        SYS_MKDIR => fs::mkdirat(aero_syscall::AT_FDCWD as _, b, c),

        _ => {
            log::error!("invalid syscall: {:#x}", a);
            Err(SyscallError::ENOSYS)
        }
    };

    aero_syscall::syscall_result_as_usize(result)
}
