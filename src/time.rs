//! The PIT (Programmable Interval Timer) chip basically consists of an oscillator, a prescaler and 3 independent frequency dividers
//! and it is used to create time intervals and calculate *estimate* time since epoch.
//!
//! **Notes**: <https://wiki.osdev.org/Programmable_Interval_Timer>

use core::time::Duration;

use crate::utils::io;

pub const PIT_DIVISOR: u64 = 1193180;
pub const PIT_BASE_FREQUENCY: u64 = 1193182;

pub struct PITDescriptor {
    ticks_since_epoch: u64,
    divisor: u64,
}

impl PITDescriptor {
    #[inline]
    const fn new() -> Self {
        Self {
            ticks_since_epoch: 0,
            divisor: 65535,
        }
    }

    pub fn sleep(&mut self, duration: Duration) {
        let start_time = self.ticks_since_epoch;
        let seconds = duration.as_secs();

        unsafe {
            while self.ticks_since_epoch < start_time + seconds {
                asm!("pause");
            }
        }
    }

    /// Increments ticks since epoch. This function is called on the PIT chip interrupt.
    #[inline]
    pub fn tick(&mut self) {
        self.ticks_since_epoch += 1 / self.get_frequency();
    }

    #[inline]
    pub fn get_frequency(&self) -> u64 {
        PIT_BASE_FREQUENCY / self.divisor
    }

    #[inline]
    pub unsafe fn set_divisor(&mut self, divisor: u64) {
        io::outb(0x40, (divisor & 0x00ff) as u8);
        io::wait();

        io::outb(0x40, ((divisor & 0xff00) >> 8) as u8);
        io::wait();
    }
}

/// The PIT (Programmable Interval Timer)
pub static mut PIT: PITDescriptor = PITDescriptor::new();

/// Initialise the PIT chip.
pub fn init() {
    unsafe {
        // At boot the PIT frequency divider is set to 0 which
        // results in around 54.926 ms between ticks.
        //
        // We change the divider to 1193180 which will have around 1.00 ms
        // between ticks to improve time accuracy.

        PIT.set_divisor(PIT_DIVISOR);
    }
}
