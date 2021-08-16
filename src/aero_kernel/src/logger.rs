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

use core::fmt::Write;

use log::{Level, LevelFilter, Metadata, Record};
use spin::Once;

use crate::utils::buffer::RingBuffer;
use crate::utils::Mutex;

const MAX_LOG_LEVEL_SPACE: usize = 6;
const DEFAULT_LOG_RING_BUFFER_SIZE: usize = 256;

static LOG_RING_BUFFER: Once<Mutex<RingBuffer<[u8; DEFAULT_LOG_RING_BUFFER_SIZE]>>> = Once::new();
static LOGGER: AeroLogger = AeroLogger;

struct AeroLogger;

impl log::Log for AeroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = record.level();
            let spaces = MAX_LOG_LEVEL_SPACE - level.as_str().len();

            macro log($($arg:tt)*) {
                $crate::prelude::serial_print!("{}", format_args!($($arg)*));
            }

            macro log_ln($($arg:tt)*) {
                $crate::prelude::serial_println!("{}", format_args!($($arg)*));
            }

            // Append the log message to the log ring buffer.
            let mut log_ring = LOG_RING_BUFFER.get().unwrap().lock_irq();
            let _ = writeln!(log_ring, "[{}] {}", level, record.args());

            log!("[ ");
            log!("{}", level);

            log_ln!("{: <1$}]        {args}", "", spaces, args = record.args());
        }
    }

    fn flush(&self) {}
}

/// Force-unlocks the logger ring buffer to prevent a deadlock.
///
/// ## Saftey
/// This method is not memory safe and should be only used when absolutely necessary.
pub unsafe fn force_unlock() {
    LOG_RING_BUFFER.get().map(|l| l.force_unlock());
}

/// Initialize the global logger instance and the logger ring
/// buffer.
pub fn init() {
    LOG_RING_BUFFER.call_once(|| Mutex::new(RingBuffer::new([0; DEFAULT_LOG_RING_BUFFER_SIZE])));

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .unwrap();
}
