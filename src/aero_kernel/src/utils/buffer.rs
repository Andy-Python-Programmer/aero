use core::fmt::Write;

/// Special special kind of buffer that stores valid UTF-8 text
/// is always a constant size, removing the oldest messages when
/// new messages are received without allocating memory on the
/// kernel heap.
#[derive(Debug)]
pub struct RingBuffer<S: AsRef<[u8]> + AsMut<[u8]>> {
    storage: S,
    position: usize,
}

impl<S: AsRef<[u8]> + AsMut<[u8]>> RingBuffer<S> {
    /// Creates a new kernel ring buffer with the storage provided.
    pub fn new(storage: S) -> Self {
        let mut this = Self {
            storage,
            position: 0,
        };

        this.clear_buffer();
        this
    }

    /// Clears the ring buffer and resets the position.
    pub fn clear_buffer(&mut self) {
        self.position = 0;

        for item in self.storage.as_mut().iter_mut() {
            /*
             * Set the item to `0xff` (non-leading UTF-8 code unit).
             */
            *item = 0xff;
        }
    }

    /// Appends the provided byte to the ring buffer.
    pub fn append_byte(&mut self, byte: u8) {
        self.storage.as_mut()[self.position] = byte;
        self.position = (self.position + 1) % self.storage.as_mut().len()
    }
}

impl<S: AsRef<[u8]> + AsMut<[u8]>> Write for RingBuffer<S> {
    fn write_str(&mut self, string: &str) -> core::fmt::Result {
        for &byte in string.as_bytes() {
            self.append_byte(byte);
        }

        Ok(())
    }
}
