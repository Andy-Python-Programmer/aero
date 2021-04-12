use crate::vga::color::*;
// use crate::vga::rendy::RENDERER;

use crate::{print, println};
use log::{Level, LevelFilter, Metadata, Record};

pub static LOGGER: AeroLogger = AeroLogger;

pub struct AeroLogger;

impl log::Log for AeroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        // if self.enabled(record.metadata()) {
        //     RENDERER.lock().color_code = ColorCode::new(Color::White, Color::Black);
        //     print!("[ ");
        //     RENDERER.lock().color_code = ColorCode::new(Color::LightGreen, Color::Black);
        //     print!("OK");
        //     RENDERER.lock().color_code = ColorCode::new(Color::White, Color::Black);
        //     println!(" ]        - {}", record.args());
        // }
    }

    fn flush(&self) {}
}

/// Initialize the logger.
pub fn init() {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .unwrap();
}
