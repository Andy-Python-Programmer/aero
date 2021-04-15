use crate::vga::color::*;
use crate::vga::rendy;

use crate::{print, println};
use log::{Level, LevelFilter, Metadata, Record};

pub static LOGGER: AeroLogger = AeroLogger;

pub struct AeroLogger;

impl log::Log for AeroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            rendy::set_color_code(ColorCode::new(0xFFFFFF, 0x00));
            print!("[ ");

            rendy::set_color_code(ColorCode::new(0x50C878, 0x00));
            print!("{}", record.level());

            rendy::set_color_code(ColorCode::new(0xFFFFFF, 0x00));
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
