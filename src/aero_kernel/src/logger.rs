// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use log::{Level, LevelFilter, Metadata, Record};
use spin::Once;

use crate::userland::scheduler;
use crate::utils::buffer::RingBuffer;
use crate::utils::sync::Mutex;

const DEFAULT_LOG_RING_BUFFER_SIZE: usize = 4096;

static LOG_RING_BUFFER: Once<Mutex<RingBuffer<[u8; DEFAULT_LOG_RING_BUFFER_SIZE]>>> = Once::new();
static LOGGER: AeroLogger = AeroLogger;

static RENDY_DEBUG: AtomicBool = AtomicBool::new(false);

struct AeroLogger;

impl log::Log for AeroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            use crate::drivers::uart::*;

            let file = record.file().unwrap_or("unknown");
            let file = file.strip_prefix("aero_kernel/src/").unwrap_or(file);

            let line = record.line().unwrap_or(0);

            let level = record.level();
            let rendy_dbg = RENDY_DEBUG.load(Ordering::Relaxed);

            macro generic_log($($arg:tt)*) {
                {
                    serial_print!("{}", format_args!($($arg)*));
                    if rendy_dbg {
                        $crate::rendy::print!("{}", format_args!($($arg)*));
                    }
                }
            }

            // Append the log message to the log ring buffer.
            let mut log_ring = LOG_RING_BUFFER.get().unwrap().lock_irq();
            let _ = writeln!(log_ring, "[{}] {}", level, record.args());

            let ticks = crate::arch::time::get_uptime_ticks();
            serial_print!("\x1b[37;1m[{}] {file}:{line} ", ticks);

            if scheduler::is_initialized() {
                // fetch the current task, grab the TID and PID.
                if let Some(task) = scheduler::get_scheduler().current_task_optional() {
                    serial_print!(
                        "(tid={}, pid={}) ",
                        task.tid().as_usize(),
                        task.pid().as_usize()
                    );
                }
            }

            match record.level() {
                Level::Info => generic_log!("\x1b[32;1minfo "), // green info
                Level::Warn => generic_log!("\x1b[33;1mwarn "), // yellow warn
                Level::Error => generic_log!("\x1b[32;1merror "), // red error
                Level::Debug => generic_log!("\x1b[35;1mdebug "), // gray debug
                Level::Trace => generic_log!("\x1b[34;1mtrace "), // blue trace
            }

            generic_log!("\x1b[0m");
            generic_log!("{}\n", record.args());
        }
    }

    fn flush(&self) {}
}

/// Force-unlocks the logger ring buffer to prevent a deadlock.
///
/// ## Safety
/// This method is not memory safe and should be only used when absolutely necessary.
#[inline]
pub unsafe fn force_unlock() {
    if let Some(l) = LOG_RING_BUFFER.get() {
        l.force_unlock()
    }
}

pub fn get_log_buffer() -> String {
    LOG_RING_BUFFER
        .get()
        .map(|l| String::from(l.lock_irq().extract()))
        .expect("log: attempted to get the log ring buffer before it was initialized")
}

#[inline]
pub fn enabled_rendy_debug() -> bool {
    RENDY_DEBUG.load(Ordering::SeqCst)
}

#[inline]
pub fn set_rendy_debug(yes: bool) {
    RENDY_DEBUG.store(yes, Ordering::SeqCst);
}

pub fn init() {
    LOG_RING_BUFFER.call_once(|| Mutex::new(RingBuffer::new([0; DEFAULT_LOG_RING_BUFFER_SIZE])));

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .unwrap();
}
