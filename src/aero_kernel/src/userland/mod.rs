use x86_64::structures::paging::OffsetPageTable;
use xmas_elf::ElfFile;

use crate::prelude::*;
use crate::syscall;

use self::process::Process;

pub mod process;
pub mod scheduler;

#[rustfmt::skip]
static USERLAND_SHELL: &[u8] = include_bytes!("../../../../userland/target/x86_64-unknown-none/debug/aero_shell");

intel_fn! {
    pub extern "asm" fn jump_userland(address: usize) {
        "
        mov ax, (6 * 8) | 3
        mov ds, ax
        mov es, ax 
        mov fs, ax 
        mov gs, ax

        mov rax, rsp
        
        push (6 * 8) | 3
        push rax
        pushf
        push (5 * 8) | 3
        push rdi

        iretq
        ",
    }
}

pub fn run(offset_table: &mut OffsetPageTable) -> Result<(), &'static str> {
    let shell_elf = ElfFile::new(USERLAND_SHELL)?;
    let shell_process = Process::from_elf(offset_table, &shell_elf);

    scheduler::get_scheduler().push(shell_process);

    Ok(())
}

/// Initialize userland.
pub fn init() {
    scheduler::init();
    syscall::init();
}
