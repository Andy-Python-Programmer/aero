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

use x86_64::structures::paging::OffsetPageTable;
use xmas_elf::ElfFile;

use crate::syscall;

use self::process::Process;

pub mod process;
pub mod scheduler;

#[rustfmt::skip]
static USERLAND_SHELL: &[u8] = include_bytes!("../../../../userland/target/x86_64-unknown-none/debug/aero_shell");

global_asm!(include_str!("threading.S"));

pub fn run(offset_table: &mut OffsetPageTable) -> Result<(), &'static str> {
    let shell_elf = ElfFile::new(USERLAND_SHELL)?;
    let shell_process = Process::from_user_elf(offset_table, &shell_elf).unwrap();

    scheduler::get_scheduler().register_process(shell_process);

    Ok(())
}

/// Initialize userland.
pub fn init() {
    scheduler::init();
    syscall::init();
}
