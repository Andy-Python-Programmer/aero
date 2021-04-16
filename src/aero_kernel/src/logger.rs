use crate::rendy;

use crate::{print, println};
use log::{Level, LevelFilter, Metadata, Record};

use aero_gfx::debug::color::{Color, ColorCode};

pub static LOGGER: AeroLogger = AeroLogger;

pub struct AeroLogger;

impl log::Log for AeroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
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

            print!("{}", record.level());

            rendy::set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));
            println!(" ]        - {}", record.args());
        }
    }

    fn flush(&self) {}
}

/// Initialize the logger.
pub fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .unwrap();
}
