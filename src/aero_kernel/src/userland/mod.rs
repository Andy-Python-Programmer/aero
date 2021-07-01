/*
 * Copyright (C) 2021 The Aero Project Developers.
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

use x86_64::structures::paging::OffsetPageTable;
use xmas_elf::ElfFile;

use crate::syscall;

pub mod scheduler;
pub mod task;

#[rustfmt::skip]
static USERLAND_SHELL: &[u8] = include_bytes!("../../../../userland/target/x86_64-unknown-none/debug/aero_shell");

#[allow(unused)]
pub fn run(offset_table: &mut OffsetPageTable) -> Result<(), &'static str> {
    let shell_elf = ElfFile::new(USERLAND_SHELL)?;
    Ok(())
}

/// Initialize userland.
pub fn init() {
    scheduler::init();
    syscall::init();
}
