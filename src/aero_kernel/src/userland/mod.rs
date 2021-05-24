/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use x86_64::{structures::paging::OffsetPageTable, VirtAddr};
use xmas_elf::ElfFile;

use crate::prelude::*;
use crate::syscall;

use self::process::Process;

pub mod context;
pub mod process;
pub mod scheduler;

#[rustfmt::skip]
static USERLAND_SHELL: &[u8] = include_bytes!("../../../../userland/target/x86_64-unknown-none/debug/aero_shell");

intel_fn! {
    /**
     * ## Notes
     * Here its is fine to use [VirtAddr] as the argument type as it is represented as a
     * transparent struct. So after compilation the argument should result in u64 instead
     * of [VirtAddr].
     */
    pub extern "asm" fn jump_userland(stack_top: VirtAddr, instruction_ptr: VirtAddr, argument: u64) {
        /*
         * After pushing all of the required registers on the stack
         * disable interrupts as we are swaping stacks. Interrupts are
         * automatically enabled after `sysretq`.
         */
        "cli\n",

        "push rdi\n", // Param: stack_top
        "push rsi\n", // Param: instruction_ptr
        "push rdx\n", // Param: rflags

        "call restore_user_tls\n",

        "pop r11\n",
        "pop rcx\n",
        "pop rsp\n",

        "fninit\n",
        "sysretq\n",
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
