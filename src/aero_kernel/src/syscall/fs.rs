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

use aero_syscall::AeroSyscallError;

use crate::{fs::Path, utils::validate_str};

pub fn write(_fd: usize, _buf: usize, _len: usize) -> Result<usize, AeroSyscallError> {
    // let current_task = scheduler::active_task_ref();

    // mem::drop(scheduler);

    // current_task
    //     .file_table
    //     .get_handle(fd)
    //     .ok_or(AeroSyscallError::EBADFD)?;

    Ok(0)
}

pub fn open(_fd: usize, path: usize, len: usize, _mode: usize) -> Result<usize, AeroSyscallError> {
    if let Some(path) = validate_str(path as *const u8, len) {
        log::debug!("{}", path);
        let _ = Path::new(path);

        Ok(0)
    } else {
        Err(AeroSyscallError::EINVAL)
    }
}
