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

use crate::fs;
use crate::fs::Path;

use crate::syscall;

pub mod scheduler;
pub mod task;
pub mod vm;

pub fn run() -> fs::Result<()> {
    let init_path = Path::new("/bin/init");
    let init_inode = fs::lookup_path(init_path)?;

    scheduler::get_scheduler().exec(init_inode, None, None);
    Ok(())
}

pub fn run_tests() -> fs::Result<()> {
    let utest_path = Path::new("/bin/utest");
    let utest_inode = fs::lookup_path(utest_path)?;

    scheduler::get_scheduler().exec(utest_inode, None, None);
    Ok(())
}

pub fn init_ap() {
    syscall::init();
}

pub fn init() {
    scheduler::init();
    syscall::init();
}
