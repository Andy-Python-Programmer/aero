//! The PIT (Programmable Interval Timer) chip basically consists of an oscillator, a prescaler and 3 independent frequency dividers
//! and it is used to create time intervals and calculate *estimate* time since epoch.
//!
//! **Notes**: <https://wiki.osdev.org/Global_Descriptor_Table>

use core::time::Duration;

pub struct PITDescriptor {
    /// CPU ticks since epoch.
    ticks_since_epoch: u64,
}

impl PITDescriptor {
    /// Create a new PIT descriptor.
    #[inline]
    const fn new() -> Self {
        Self {
            ticks_since_epoch: 0,
        }
    }

    // TODO: Calculate the most accurate time.
    pub fn sleep(&mut self, duration: Duration) {
        let start_time = self.ticks_since_epoch;
        let seconds = duration.as_secs();

        unsafe {
            while self.ticks_since_epoch < start_time + seconds {
                asm!("hlt");
            }
        }
    }

    /// Increments ticks since epoch. This function is called on the PIT chip interrupt.
    pub fn tick(&mut self) {
        self.ticks_since_epoch += 1;
    }

    /// Get the CPU ticks since epoch.
    pub fn get_ticks_since_epoch(&self) -> u64 {
        self.ticks_since_epoch
    }
}

/// The PIT (Programmable Interval Timer)
pub static mut PIT: PITDescriptor = PITDescriptor::new();

/// Initialise the PIT chip.
pub fn init() {}
