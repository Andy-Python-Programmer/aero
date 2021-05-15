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

            macro log($($arg:tt)*) {
                $crate::prelude::print!("{}", format_args!($($arg)*));
                $crate::prelude::serial_print!("{}", format_args!($($arg)*));
            }

            macro log_ln($($arg:tt)*) {
                $crate::prelude::println!("{}", format_args!($($arg)*));
                $crate::prelude::serial_println!("{}", format_args!($($arg)*));
            }

            /*
             * First of all append the log message to the log ring buffer.
             */
            let mut log_ring = get_log_ring_buffer();
            let _ = writeln!(log_ring, "[{}] {}", level, record.args());

            rendy::set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));
            log!("[ ");

            match record.level() {
                Level::Error => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0xDFF2800), Color::BLACK))
                }

                Level::Warn => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0xFFD300), Color::BLACK))
                }

                Level::Info => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0x50C878), Color::BLACK))
                }

                Level::Debug | Level::Trace => {
                    rendy::set_color_code(ColorCode::new(Color::from_hex(0x10A5F5), Color::BLACK))
                }
            }

            log!("{}", level);

            rendy::set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));
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

/// Initialize the global logger instance and the logger ring
/// buffer.
pub fn init() {
    LOG_RING_BUFFER.call_once(|| Mutex::new(RingBuffer::new([0; DEFAULT_LOG_RING_BUFFER_SIZE])));

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .unwrap();
}
