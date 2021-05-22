use x86_64::{structures::paging::OffsetPageTable, VirtAddr};
use xmas_elf::ElfFile;

use crate::arch::gdt::GdtEntryType;
use crate::syscall;

use self::process::Process;

pub mod process;
pub mod scheduler;

#[rustfmt::skip]
static USERLAND_SHELL: &[u8] = include_bytes!("../../../../userland/target/x86_64-unknown-none/debug/aero_shell");

#[inline(never)]
pub unsafe fn jump_userland(address: VirtAddr, stack_top: VirtAddr) {
    asm!(
        "\
        push rax
        push rsi
        push 0x200
        push rdx
        push rdi
        iretq
        ",
        in("rdi") address.as_u64(),
        in("rsi") stack_top.as_u64(),

        in("dx") (GdtEntryType::USER_DATA * 8) | 3,
        in("ax") (GdtEntryType::USER_CODE * 8) | 3,

        options(noreturn)
    );
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
