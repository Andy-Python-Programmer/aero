use core::fmt::Write;

use aero_gfx::debug::color::ColorCode;
use aero_gfx::debug::rendy::DebugRendy;

use log::{Level, Metadata, Record};
use spin::{Mutex, Once};

pub static LOGGER: Once<LockedLogger> = Once::new();

/// The global boot logger instance protected by a mutex.
///
/// We need a custom logger here instead of using the UEFI services prebuilt
/// logger as it can only log until we are in boot services.
pub struct LockedLogger<'buffer>(Mutex<DebugRendy<'buffer>>);

impl<'buffer> LockedLogger<'buffer> {
    #[inline(always)]
    pub fn new(mut inner: DebugRendy<'buffer>) -> Self {
        inner.clear_screen();

        Self(Mutex::new(inner))
    }
}

impl<'buffer> log::Log for LockedLogger<'buffer> {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let this = &mut *self.0.lock();

            this.set_color_code(ColorCode::new(0xFFFFFF, 0x00));
            write!(this, "[ ").expect("Failed to write to the framebuffer");

            match record.level() {
                Level::Error => this.set_color_code(ColorCode::new(0xDFF2800, 0x00)),
                Level::Warn => this.set_color_code(ColorCode::new(0xFFD300, 0x00)),
                Level::Info => this.set_color_code(ColorCode::new(0x50C878, 0x00)),
                Level::Debug | Level::Trace => this.set_color_code(ColorCode::new(0x10A5F5, 0x00)),
            }

            write!(this, "{}", record.level()).expect("Failed to write to the framebuffer");

            this.set_color_code(ColorCode::new(0xFFFFFF, 0x00));
            writeln!(this, " ]        - {}", record.args())
                .expect("Failed to write to the framebuffer");
        }
    }

    fn flush(&self) {}
}
