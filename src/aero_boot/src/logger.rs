use core::fmt::Write;

use aero_gfx::debug::color::{Color, ColorCode};
use aero_gfx::debug::rendy::DebugRendy;

use log::{Level, Metadata, Record};
use spin::{Mutex, Once};

pub static LOGGER: Once<LockedLogger> = Once::new();

/// The global boot logger instance protected by a mutex.
///
/// We need a custom logger here instead of using the UEFI services prebuilt
/// logger as it can only log until we are in boot services.
pub struct LockedLogger(Mutex<DebugRendy>);

impl LockedLogger {
    #[inline(always)]
    pub fn new(mut inner: DebugRendy) -> Self {
        inner.clear_screen();

        Self(Mutex::new(inner))
    }
}

impl log::Log for LockedLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let this = &mut *self.0.lock();

            this.set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));
            write!(this, "[ ").expect("Failed to write to the framebuffer");

            match record.level() {
                Level::Error => {
                    this.set_color_code(ColorCode::new(Color::from_hex(0xDFF2800), Color::BLACK))
                }

                Level::Warn => {
                    this.set_color_code(ColorCode::new(Color::from_hex(0xFFD300), Color::BLACK))
                }

                Level::Info => {
                    this.set_color_code(ColorCode::new(Color::from_hex(0x50C878), Color::BLACK))
                }

                Level::Debug | Level::Trace => {
                    this.set_color_code(ColorCode::new(Color::from_hex(0x10A5F5), Color::BLACK))
                }
            }

            write!(this, "{}", record.level()).expect("Failed to write to the framebuffer");

            this.set_color_code(ColorCode::new(Color::WHITE, Color::BLACK));

            writeln!(this, " ]        - {}", record.args())
                .expect("Failed to write to the framebuffer");
        }
    }

    fn flush(&self) {}
}
