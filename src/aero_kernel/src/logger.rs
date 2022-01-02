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

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use alloc::string::String;
use log::{Level, LevelFilter, Metadata, Record};
use spin::Once;

use crate::utils::buffer::RingBuffer;
use crate::utils::sync::Mutex;

const MAX_LOG_LEVEL_SPACE: usize = 6;
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
            use crate::drivers::uart_16550::*;

            if let Some(pp) = record.module_path() {
                // Only log the vm logs if the vmlog feature is enabled ;^).
                if pp == "aero_kernel::userland::vm" && !cfg!(feature = "vmlog") {
                    return;
                }
            }

            let level = record.level();
            let spaces = MAX_LOG_LEVEL_SPACE - level.as_str().len();
            let rendy_dbg = RENDY_DEBUG.load(Ordering::Relaxed);

            macro log($($arg:tt)*) {
                $crate::drivers::uart_16550::serial_print!("{}", format_args!($($arg)*));
                if rendy_dbg { $crate::rendy::print!("{}", format_args!($($arg)*)); }
            }

            macro log_ln($($arg:tt)*) {
                serial_println!("{}", format_args!($($arg)*));
                if rendy_dbg { $crate::rendy::println!("{}", format_args!($($arg)*)); }
            }

            // Append the log message to the log ring buffer.
            let mut log_ring = LOG_RING_BUFFER.get().unwrap().lock_irq();
            let _ = writeln!(log_ring, "[{}] {}", level, record.args());

            match record.level() {
                Level::Error => serial_print!("\x1b[1;41m"), // bold red
                Level::Warn => serial_print!("\x1b[1;43m"),  // bold yellow
                Level::Info => serial_print!("\x1b[1;42m"),  // bold green
                Level::Debug => serial_print!("\x1b[1;44m"), // bold blue
                Level::Trace => serial_print!("\x1b[1;45m"), // bold magenta
            }

            log!("  {}{: <2$} ", level, "", spaces);
            crate::drivers::uart_16550::serial_print!("\x1b[0;0m");

            log_ln!("      {}", record.args());
        }
    }

    fn flush(&self) {}
}

/// Force-unlocks the logger ring buffer to prevent a deadlock.
///
/// ## Saftey
/// This method is not memory safe and should be only used when absolutely necessary.
#[inline]
pub unsafe fn force_unlock() {
    LOG_RING_BUFFER.get().map(|l| l.force_unlock());
}

pub fn get_log_buffer<'a>() -> String {
    LOG_RING_BUFFER
        .get()
        .map(|l| String::from(l.lock_irq().extract()))
        .expect("log: attempted to get the log ring buffer before it was initialized")
        .clone()
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
