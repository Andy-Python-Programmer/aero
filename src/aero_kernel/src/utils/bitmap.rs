// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use core::alloc::Allocator;

use alloc::vec::Vec;
use bit_field::BitField;

pub const BLOCK_BITS: usize = core::mem::size_of::<usize>() * 8;

const fn calculate_blocks(bits: usize) -> usize {
    if bits % BLOCK_BITS == 0 {
        bits / BLOCK_BITS
    } else {
        bits / BLOCK_BITS + 1
    }
}

#[derive(Debug)]
pub struct Bitmap<A: Allocator> {
    bitmap: Vec<usize, A>,
}

impl<A: Allocator> Bitmap<A> {
    /// Constructs a new bitmap with `size` bits and uses `alloc`
    /// as the allocator.
    ///
    /// ## Example
    /// ```rust
    /// use alloc::alloc::Global;
    ///
    /// let mut bitmap = Bitmap::new_in(Global, 4096);
    /// ```
    pub fn new_in(alloc: A, size: usize) -> Self {
        let bitmap_blocks = calculate_blocks(size);
        let mut bitmap = Vec::new_in(alloc);

        bitmap.resize(bitmap_blocks, 0);
        Self { bitmap }
    }

    /// Constructs a new, empty bitmap. This function does *not* perform
    /// any allocations.
    ///
    /// ## Example
    /// ```rust
    /// let bitmap = Bitmap::new();
    /// ```
    pub fn empty(alloc: A) -> Self {
        Self {
            bitmap: Vec::new_in(alloc),
        }
    }

    /// Sets the bit at the provided `bit_idx` to `yes` (`true` or `false`).
    ///
    /// ## Example
    /// ```rust
    /// use alloc::alloc::Global;
    ///
    /// let mut bitmap = Bitmap::new_in(Global, 4096);
    ///
    /// assert!(!bitmap.is_set(69));
    /// bitmap.set(69, true);
    /// assert!(bitmap.is_set(69));
    /// ```
    pub fn set(&mut self, bit_idx: usize, yes: bool) {
        let (block_idx, mod_bit_idx) = (bit_idx / BLOCK_BITS, bit_idx % BLOCK_BITS);

        self.bitmap
            .get_mut(block_idx)
            .map(|n| n.set_bit(mod_bit_idx, yes));
    }

    /// Returns weather the bit at the provided `bit_idx` is set.
    ///
    /// ## Example
    /// ```rust
    /// use alloc::alloc::Global;
    ///
    /// let bitmap = Bitmap::new_in(Global, 4096);
    /// assert!(!bitmap.is_set(69));
    /// ```
    pub fn is_set(&self, bit_idx: usize) -> bool {
        let (block_idx, mod_bit_idx) = (bit_idx / BLOCK_BITS, bit_idx % BLOCK_BITS);
        let n = self.bitmap[block_idx];

        n.get_bit(mod_bit_idx)
    }

    /// Returns the index of the first unset bit.
    ///
    /// ## Example
    /// ```rust
    /// use alloc::alloc::Global;
    ///
    /// let mut bitmap = Bitmap::new_in(Global, 4096);
    ///
    /// bitmap.set(69, true);
    /// assert_eq!(bitmap.find_first_set(), Some(0));
    /// ```
    pub fn find_first_unset(&self) -> Option<usize> {
        for (i, block) in self.bitmap.iter().enumerate() {
            let trailing_ones = block.trailing_ones();
            if trailing_ones < BLOCK_BITS as u32 {
                return Some(i * BLOCK_BITS + trailing_ones as usize);
            }
        }

        None
    }

    /// Returns the index of the first set bit.
    ///
    /// ## Example
    /// ```rust
    /// use alloc::alloc::Global;
    ///
    /// let mut bitmap = Bitmap::new_in(Global, 4096);
    ///
    /// bitmap.set(69, true);
    /// assert_eq!(bitmap.find_first_set(), Some(69));
    /// ```
    pub fn find_first_set(&self) -> Option<usize> {
        for (i, block) in self.bitmap.iter().enumerate() {
            let trailing_zeros = block.trailing_zeros();
            if trailing_zeros < BLOCK_BITS as u32 {
                return Some(i * BLOCK_BITS + trailing_zeros as usize);
            }
        }

        None
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::alloc::Global;

    const TEST_BITMAP_SIZE: usize = 4096;

    #[test]
    fn find_first_unset() {
        let mut map = Bitmap::new_in(Global, TEST_BITMAP_SIZE);

        // Set all of the bits to true.
        for i in 0..TEST_BITMAP_SIZE {
            assert_eq!(map.find_first_unset(), Some(i));
            map.set(i, true);
        }

        assert_eq!(map.find_first_unset(), None);

        map.set(0, false);
        assert_eq!(map.find_first_unset(), Some(0));

        map.set(0, true);
        map.set(1, false);
        assert_eq!(map.find_first_unset(), Some(1));

        map.set(56, false);
        assert_eq!(map.find_first_unset(), Some(1));

        map.set(1, true);
        assert_eq!(map.find_first_unset(), Some(56));

        map.set(80, false);
        assert_eq!(map.find_first_unset(), Some(56));

        map.set(56, true);
        assert_eq!(map.find_first_unset(), Some(80));

        map.set(82, false);
        assert_eq!(map.find_first_unset(), Some(80));

        map.set(80, true);
        assert_eq!(map.find_first_unset(), Some(82));

        map.set(5, false);
        assert_eq!(map.find_first_unset(), Some(5));
    }

    #[test]
    fn find_first_set() {
        let mut map = Bitmap::new_in(Global, TEST_BITMAP_SIZE);
        assert_eq!(map.find_first_set(), None);

        map.set(0, true);
        assert_eq!(map.find_first_set(), Some(0));

        map.set(0, false);
        map.set(1, true);
        assert_eq!(map.find_first_set(), Some(1));

        map.set(56, true);
        assert_eq!(map.find_first_set(), Some(1));

        map.set(1, false);
        assert_eq!(map.find_first_set(), Some(56));

        map.set(80, true);
        assert_eq!(map.find_first_set(), Some(56));

        map.set(56, false);
        assert_eq!(map.find_first_set(), Some(80));

        map.set(82, true);
        assert_eq!(map.find_first_set(), Some(80));

        map.set(80, false);
        assert_eq!(map.find_first_set(), Some(82));

        map.set(5, true);
        assert_eq!(map.find_first_set(), Some(5));
    }
}
