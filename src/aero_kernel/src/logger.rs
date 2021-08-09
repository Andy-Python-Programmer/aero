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

use crate::rendy::{Color, ColorCode};
use log::{Level, LevelFilter, Metadata, Record};
use spin::{Mutex, MutexGuard, Once};

use crate::rendy;
use crate::utils::buffer::RingBuffer;

const MAX_LOG_LEVEL_SPACE: usize = 6;
const DEFAULT_LOG_RING_BUFFER_SIZE: usize = 256;

static LOG_RING_BUFFER: Once<Mutex<RingBuffer<[u8; DEFAULT_LOG_RING_BUFFER_SIZE]>>> = Once::new();
static LOGGER: AeroLogger = AeroLogger;

struct AeroLogger;

impl log::Log for AeroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = record.level();
            let spaces = MAX_LOG_LEVEL_SPACE - level.as_str().len();

            /*
             * Helper variable to track if the rendy was initialized when this function was invoked as
             * we do not like to panic when we call log before the initialization because in that case
             * we log to the serial (COM 1) output.
             */
            let initialized = rendy::is_initialized();

            macro log($($arg:tt)*) {
                if initialized { $crate::prelude::print!("{}", format_args!($($arg)*)); }
                $crate::prelude::serial_print!("{}", format_args!($($arg)*));
            }

            macro log_ln($($arg:tt)*) {
                if initialized { $crate::prelude::println!("{}", format_args!($($arg)*)); }
                $crate::prelude::serial_println!("{}", format_args!($($arg)*));
            }

            /*
             * First of all append the log message to the log ring buffer.
             */
            let mut log_ring = get_log_ring_buffer();
            let _ = writeln!(log_ring, "[{}] {}", level, record.args());

            if initialized {
                rendy::set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));
            }

            log!("[ ");

            match record.level() {
                Level::Error if initialized => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0xDFF2800), Color::BLACK))
                }

                Level::Warn if initialized => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0xFFD300), Color::BLACK))
                }

                Level::Info if initialized => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0x50C878), Color::BLACK))
                }

                Level::Debug | Level::Trace if initialized => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0x10A5F5), Color::BLACK))
                }

                _ => {}
            }

            log!("{}", level);

            if initialized {
                rendy::set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));
            }

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

/// Returns a mutable reference to the logging ring buffer.
fn get_log_ring_buffer() -> MutexGuard<'static, RingBuffer<[u8; DEFAULT_LOG_RING_BUFFER_SIZE]>> {
    LOG_RING_BUFFER
        .get()
        .expect("Attempted to get the logging ring buffer before it was initialized")
        .lock()
}

/// Initialize the global logger instance and the logger ring
/// buffer.
pub fn init() {
    LOG_RING_BUFFER.call_once(|| Mutex::new(RingBuffer::new([0; DEFAULT_LOG_RING_BUFFER_SIZE])));

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Debug))
        .unwrap();
}
