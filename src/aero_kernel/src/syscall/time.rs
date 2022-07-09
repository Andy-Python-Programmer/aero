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

use aero_syscall::{SyscallError, TimeSpec};

use crate::userland::scheduler;
use crate::utils::CeilDiv;

const CLOCK_TYPE_REALTIME: usize = 0;
const CLOCK_TYPE_MONOTONIC: usize = 1;

#[syscall]
pub fn sleep(timespec: &TimeSpec) -> Result<usize, SyscallError> {
    let duration = (timespec.tv_nsec as usize).ceil_div(1000000000) + timespec.tv_sec as usize;

    scheduler::get_scheduler().inner.sleep(Some(duration))?;

    Ok(0x00)
}

#[syscall]
pub fn gettime(clock: usize, timespec: &mut TimeSpec) -> Result<usize, SyscallError> {
    match clock {
        CLOCK_TYPE_REALTIME => {
            let clock = crate::time::get_realtime_clock();

            timespec.tv_sec = clock.tv_sec;
            timespec.tv_nsec = clock.tv_nsec;

            Ok(0x00)
        }

        CLOCK_TYPE_MONOTONIC => {
            // FIXME: implement
            let clock = crate::time::get_realtime_clock();

            timespec.tv_sec = clock.tv_sec;
            timespec.tv_nsec = clock.tv_nsec;

            Ok(0x00)
        }

        _ => Err(SyscallError::EINVAL),
    }
}
