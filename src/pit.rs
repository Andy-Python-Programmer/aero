use core::time::Duration;

pub struct PITDescriptor {
    pub ticks_since_epoch: u64,
}

impl PITDescriptor {
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

    pub fn tick(&mut self) {
        self.ticks_since_epoch += 1;
    }
}

pub static mut PIT: PITDescriptor = PITDescriptor::new();

/// Initialise the PIT chip.
pub fn init() {}
