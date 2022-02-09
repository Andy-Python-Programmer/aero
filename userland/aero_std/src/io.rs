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

use aero_syscall::*;
use core::fmt::{Result as FmtResult, Write};

pub struct Stdout;
pub struct Stderr;

impl Write for Stdout {
    fn write_str(&mut self, str: &str) -> FmtResult {
        sys_write(1, str.as_bytes()).unwrap();

        Ok(())
    }
}

impl Write for Stderr {
    fn write_str(&mut self, str: &str) -> FmtResult {
        sys_write(2, str.as_bytes()).unwrap();

        Ok(())
    }
}

/// This function tells the current value of the file position indicator for the provided
/// file descriptor (`fd`).
pub fn tell(fd: usize) -> Result<usize, AeroSyscallError> {
    sys_seek(fd, 0, SeekWhence::SeekCur)
}
