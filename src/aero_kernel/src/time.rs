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

//! The PIT (Programmable Interval Timer) chip basically consists of an oscillator,
//! a prescaler and 3 independent frequency dividers and it is used to create time intervals
//! and calculate *estimate* time since epoch.
//!
//! **Notes**: <https://wiki.osdev.org/Programmable_Interval_Timer>

use core::sync::atomic::{AtomicUsize, Ordering};

use aero_syscall::TimeSpec;
use stivale_boot::v2::StivaleEpochTag;

use crate::apic;
use crate::userland::scheduler;
use crate::utils::io;
use crate::utils::sync::Mutex;

const PIT_FREQUENCY_HZ: usize = 1000;
pub const PIT_DIVIDEND: usize = 1193182;

const SCHED_TIMESLICE_MS: usize = 15;

static SCHED_TICKS: AtomicUsize = AtomicUsize::new(0);
static UPTIME_RAW: AtomicUsize = AtomicUsize::new(0);
static UPTIME_SEC: AtomicUsize = AtomicUsize::new(0);

pub static EPOCH_TAG: spin::Once<&'static StivaleEpochTag> = spin::Once::new();
pub static REALTIME_CLOCK: Mutex<aero_syscall::TimeSpec> = Mutex::new(aero_syscall::TimeSpec {
    tv_sec: 0,
    tv_nsec: 0,
});

pub fn get_uptime_ticks() -> usize {
    UPTIME_SEC.load(Ordering::SeqCst)
}

pub fn tick() {
    {
        let interval = aero_syscall::TimeSpec {
            tv_sec: 0,
            tv_nsec: (1000000000 / PIT_FREQUENCY_HZ) as isize,
        };

        let mut this = REALTIME_CLOCK.lock();

        if this.tv_nsec + interval.tv_nsec > 999999999 {
            let diff = (this.tv_nsec + interval.tv_nsec) - 1000000000;

            this.tv_nsec = diff;
            this.tv_sec += 1;
        } else {
            this.tv_nsec += interval.tv_nsec
        }

        this.tv_sec += interval.tv_sec;
    }

    let value = UPTIME_RAW.fetch_add(1, Ordering::Relaxed); // Increment uptime raw ticks.

    if value % PIT_FREQUENCY_HZ == 0 {
        UPTIME_SEC.fetch_add(1, Ordering::Relaxed); // Increment uptime seconds
    }

    let value = SCHED_TICKS.fetch_add(1, Ordering::Relaxed); // Increment scheduler ticks.

    // Check if the ticks are equal to the scheduler timeslice. If so, then
    // reschedule.
    if value == SCHED_TIMESLICE_MS {
        SCHED_TICKS.store(0, Ordering::Relaxed); // Reset the scheduler ticks counter.

        scheduler::get_scheduler().inner.preempt();
        return;
    }
}

pub fn get_realtime_clock() -> TimeSpec {
    REALTIME_CLOCK.lock().clone()
}

/// Returns the current amount of PIT ticks.
pub fn get_current_count() -> u16 {
    unsafe {
        io::outb(0x43, 0);

        let lower = io::inb(0x40) as u16;
        let higher = io::inb(0x40) as u16;

        (higher << 8) | lower
    }
}

pub fn set_reload_value(new_count: u16) {
    // Channel 0, lo/hi access mode, mode 2 (rate generator)
    unsafe {
        io::outb(0x43, 0x34);
        io::outb(0x40, new_count as u8);
        io::outb(0x40, (new_count >> 8) as u8);
    }
}

pub fn set_frequency(frequency: usize) {
    let mut new_divisor = PIT_DIVIDEND / frequency;

    if PIT_DIVIDEND % frequency > frequency / 2 {
        new_divisor += 1;
    }

    set_reload_value(new_divisor as u16);
}

/// This function is responsible for initializing the PIT chip and setting
/// up the IRQ.
pub fn init() {
    REALTIME_CLOCK.lock().tv_sec = EPOCH_TAG
        .get()
        .expect("failed to initialize realtime clock")
        .epoch as isize;

    set_frequency(PIT_FREQUENCY_HZ);

    apic::get_local_apic().timer_calibrate();
    apic::io_apic_setup_legacy_irq(0, 1); // Set up the IRQ.
}
