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

use raw_cpuid::CpuId;

pub mod fs;
pub mod process;
pub mod time;

pub use fs::*;
pub use process::*;
pub use time::*;

use crate::utils::io;

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
    const NULL: Self = Self(sys_unimplemented as *const _);

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
        Self([SyscallEntry::NULL; SYSCALL_TABLE_LENGTH])
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
    let function_info = CpuId::new()
        .get_extended_function_info()
        .expect("Failed to retrieve CPU function info");

    unsafe {
        // Initialize the syscall table.

        SYSCALL_HANDLER_TABLE[SYS_EXIT].set_function(process::exit as *const _);

        // Enable support for `syscall` and `sysret` instructions if the current
        // CPU supports them.
        if function_info.has_syscall_sysret() {
            let efer = io::rdmsr(io::IA32_EFER);

            io::wrmsr(io::IA32_EFER, efer | 1);

            io::wrmsr(io::IA32_STAR, 0x0013_0008_0000_0000);
            io::wrmsr(io::IA32_LSTAR, syscall_handler as u64);

            // Clear the trap flag and enable interrupts.
            io::wrmsr(io::IA32_FMASK, 0x300);
        }
    }
}
