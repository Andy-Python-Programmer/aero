/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

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
            // Set the item to `0xff` (non-leading UTF-8 code unit).
            *item = 0xff;
        }
    }

    fn rotate(&mut self) {
        self.storage.as_mut().rotate_left(self.position);
        self.position = 0;
    }

    /// Extracts the contents of the ring buffer as a string slice, excluding any
    /// partially overwritten UTF-8 code unit sequences at the beginning.
    ///
    /// Extraction rotates the contents of the ring buffer such that all of its
    /// contents becomes contiguous in memory.
    ///
    /// This function takes O(n) time where n is buffer length.
    pub fn extract(&mut self) -> &str {
        self.rotate();

        let is_utf8_leader = |byte: u8| -> bool {
            byte & 0b10000000 == 0b00000000
                || byte & 0b11100000 == 0b11000000
                || byte & 0b11110000 == 0b11100000
                || byte & 0b11111000 == 0b11110000
        };

        let buffer = self.storage.as_mut();

        for i in 0..buffer.len() {
            // Chop off any non-leading UTF-8 code units at the start.
            if is_utf8_leader(buffer[i]) {
                return unsafe { core::str::from_utf8_unchecked(&buffer[i..]) };
            }
        }

        return "";
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

#[cfg(test)]
mod tests {
    use super::*;

    #[aero_test::test]
    fn partially_overwritten_buffer() {
        let mut buf = RingBuffer::new([0u8; 16]);
        write!(buf, "\nfirst\n").unwrap();
        write!(buf, "second\n").unwrap();
        write!(buf, "third\n").unwrap();

        assert_eq!(buf.extract(), "st\nsecond\nthird\n");
    }

    #[aero_test::test]
    fn clear_buffer() {
        let mut buf = RingBuffer::new([0u8; 5]);
        write!(buf, "hello").unwrap();

        assert_eq!(buf.extract(), "hello");
        buf.clear_buffer();

        assert_eq!(buf.extract(), "");
    }
}
