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
use core::sync::atomic::{AtomicBool, Ordering};

use log::{Level, LevelFilter, Metadata, Record};
use spin::Once;

use crate::utils::buffer::RingBuffer;
use crate::utils::sync::Mutex;

const MAX_LOG_LEVEL_SPACE: usize = 6;
const DEFAULT_LOG_RING_BUFFER_SIZE: usize = 256;

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
            let level = record.level();
            let spaces = MAX_LOG_LEVEL_SPACE - level.as_str().len();
            let rendy_dbg = RENDY_DEBUG.load(Ordering::Relaxed);

            macro log($($arg:tt)*) {
                $crate::prelude::serial_print!("{}", format_args!($($arg)*));
                if rendy_dbg { $crate::prelude::print!("{}", format_args!($($arg)*)); }
            }

            macro log_ln($($arg:tt)*) {
                $crate::prelude::serial_println!("{}", format_args!($($arg)*));
                if rendy_dbg { $crate::prelude::println!("{}", format_args!($($arg)*)); }
            }

            // Append the log message to the log ring buffer.
            let mut log_ring = LOG_RING_BUFFER.get().unwrap().lock_irq();
            let _ = writeln!(log_ring, "[{}] {}", level, record.args());

            match record.level() {
                Level::Error => crate::prelude::serial_print!("\x1b[1;41m"), // bold red
                Level::Warn => crate::prelude::serial_print!("\x1b[1;43m"),  // bold yellow
                Level::Info => crate::prelude::serial_print!("\x1b[1;42m"),  // bold green
                Level::Debug => crate::prelude::serial_print!("\x1b[1;44m"), // bold blue
                Level::Trace => crate::prelude::serial_print!("\x1b[1;45m"), // bold magenta
            }

            log!("  {}{: <2$} ", level, "", spaces);
            log_ln!("\x1b[0;0m      {}", record.args());
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
