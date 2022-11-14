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

use aero_syscall::time::{ITimerVal, ITIMER_REAL};
use aero_syscall::{SyscallError, TimeSpec};
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::userland::scheduler;
use crate::userland::task::Task;
use crate::utils::sync::{IrqGuard, Mutex};
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
            let clock = crate::arch::time::get_realtime_clock();

            timespec.tv_sec = clock.tv_sec;
            timespec.tv_nsec = clock.tv_nsec;

            Ok(0x00)
        }

        CLOCK_TYPE_MONOTONIC => {
            // FIXME: implement
            let clock = crate::arch::time::get_realtime_clock();

            timespec.tv_sec = clock.tv_sec;
            timespec.tv_nsec = clock.tv_nsec;

            Ok(0x00)
        }

        _ => Err(SyscallError::EINVAL),
    }
}

static TIMERS: Mutex<Vec<Arc<Task>>> = Mutex::new(Vec::new());

pub fn check_timers() {
    // if let Some(task) = TIMERS.lock_irq().pop() {
    //     task.signal(aero_syscall::signal::SIGALRM);
    // }
}

#[syscall]
pub fn setitimer(
    which: usize,
    _new_value: &ITimerVal,
    _old_value: usize, // FIXME: Option<&mut ITimerVal>
) -> Result<usize, SyscallError> {
    let _guard = IrqGuard::new();

    match which {
        // The interval timer value is decremented in real time. The SIGALRM signal is
        // generated for the process when this timer expires.
        ITIMER_REAL => {}

        _ => unreachable!("setitimer: unimplemented timer (ty={which})"),
    }

    TIMERS
        .lock_irq()
        .push(scheduler::get_scheduler().current_task());

    Ok(0)
}

#[syscall]
pub fn getitimer(_which: usize, _curr_value: &mut ITimerVal) -> Result<usize, SyscallError> {
    Ok(0)
}
