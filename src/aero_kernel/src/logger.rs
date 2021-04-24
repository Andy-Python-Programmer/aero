use crate::rendy;

use crate::{print, println};
use log::{Level, LevelFilter, Metadata, Record};

use aero_gfx::debug::color::{Color, ColorCode};

static LOGGER: AeroLogger = AeroLogger;

const MAX_LOG_LEVEL_SPACE: usize = 6;

struct AeroLogger;

impl log::Log for AeroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level = record.level();

            rendy::set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));
            print!("[ ");

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

            print!("{}", level);

            rendy::set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));

            let spaces = MAX_LOG_LEVEL_SPACE - level.as_str().len();

            println!("{: <1$}]        {args}", "", spaces, args = record.args());
        }
    }

    fn flush(&self) {}
}

/// Initialize the global logger instance.
pub fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Trace))
        .unwrap();
}

#[no_mangle]
extern "C" fn log_debug(_: *const u8) {
    log::debug!("(asm)")
}
