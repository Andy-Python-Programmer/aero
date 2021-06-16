/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use core::fmt::Write;

use aero_gfx::debug::color::{Color, ColorCode};
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
        metadata.level() <= Level::Trace
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

/// Returns a mutable reference to the logging ring buffer.
fn get_log_ring_buffer() -> MutexGuard<'static, RingBuffer<[u8; DEFAULT_LOG_RING_BUFFER_SIZE]>> {
    LOG_RING_BUFFER
        .get()
        .expect("Attempted to get the logging ring buffer before it was initialized")
        .lock()
}

#[no_mangle]
extern "C" fn log_debug() {
    log::debug!("(asm)");
}

/// Initialize the global logger instance and the logger ring
/// buffer.
pub fn init() {
    LOG_RING_BUFFER.call_once(|| Mutex::new(RingBuffer::new([0; DEFAULT_LOG_RING_BUFFER_SIZE])));

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .unwrap();
}
