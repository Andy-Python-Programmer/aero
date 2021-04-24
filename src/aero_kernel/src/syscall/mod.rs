//! System Calls are used to call a kernel service from user land.
//!
//! | %rax   | Name                    |
//! |--------|-------------------------|
//! | 0      | read                    |
//! | 1      | write                   |
//! | 2      | open                    |
//! | 3      | close                   |
//! | 60     | exit                    |
//!
//! **Notes**: <https://wiki.osdev.org/System_Calls>

use core::ops::{Index, IndexMut};

pub mod fs;
pub mod process;
pub mod time;

pub use fs::*;
pub use process::*;
pub use time::*;

pub const SYSCALL_TABLE_LENGTH: usize = 313;

pub const SYS_EXIT: usize = 60;

extern "C" {
    pub fn syscall_handler();
    pub fn sys_unimplemented();
}

#[no_mangle]
static mut SYSCALL_HANDLER_TABLE: SyscallTable = SyscallTable::new();

#[repr(transparent)]
struct SyscallEntry(*const usize);

impl SyscallEntry {
    #[inline(always)]
    const fn null() -> Self {
        Self(sys_unimplemented as *const _)
    }

    #[inline(always)]
    pub fn set_function(&mut self, handler: *const usize) {
        self.0 = handler;
    }
}

#[repr(C, align(0x40))]
struct SyscallTable([SyscallEntry; SYSCALL_TABLE_LENGTH]);

impl SyscallTable {
    #[inline]
    const fn new() -> Self {
        const NULL_SYSCALL_ENTRY: SyscallEntry = SyscallEntry::null();

        Self([NULL_SYSCALL_ENTRY; SYSCALL_TABLE_LENGTH])
    }
}

impl Index<usize> for SyscallTable {
    type Output = SyscallEntry;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for SyscallTable {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

unsafe impl Send for SyscallTable {}
unsafe impl Sync for SyscallTable {}

pub fn init() {
    unsafe {
        SYSCALL_HANDLER_TABLE[SYS_EXIT].set_function(process::exit as *const _);
    }
}
