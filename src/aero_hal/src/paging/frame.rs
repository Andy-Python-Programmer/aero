/*
 * Copyright (C) 2021 The Aero Project Developers.
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

use core::fmt;
use core::marker::PhantomData;

use super::{address::PhysicalAddress, PageSize, Size4KiB};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct Frame<S: PageSize = Size4KiB> {
    start_address: PhysicalAddress,
    size: PhantomData<S>,
}

impl<S: PageSize> Frame<S> {
    pub const SIZE: u64 = S::SIZE;

    pub fn from_start_address(address: PhysicalAddress) -> Self {
        assert!(address.is_aligned(S::SIZE));

        Self::containing_address(address)
    }

    #[inline]
    pub fn containing_address(address: PhysicalAddress) -> Self {
        Self {
            start_address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    #[inline(always)]
    pub fn start_address(&self) -> PhysicalAddress {
        self.start_address
    }
}

impl<S: PageSize> fmt::Debug for Frame<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "Frame[{}]({:#x})",
            S::SIZE,
            self.start_address().as_u64()
        ))
    }
}

pub trait FrameAllocator<S: PageSize> {
    fn allocate_frame(&mut self) -> Option<Frame<S>>;
    fn deallocate_frame(&mut self);
}
