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

use alloc::boxed::Box;

use core::alloc::{AllocError, Allocator, Layout};
use core::mem::MaybeUninit;
use core::ptr::NonNull;

use crate::mem::paging::*;

pub struct DmaAllocator;

/*
 * XXX: The main issue with DMA buffers is when they are bigger than one page. DMA buffers
 * must be made of contiguous pages in physical memory because the device transfers the
 * the data using the ISA or PCI system bus (which carry physical addresses).
 */
unsafe impl Allocator for DmaAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // XXX: The DMA buffer must be aligned to a page boundary.
        let size_bytes = layout.size() as u64;

        let phys = if size_bytes <= Size4KiB::SIZE {
            let frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR.allocate_frame().ok_or(AllocError)?;
            frame.start_address()
        } else {
            assert!(size_bytes <= Size2MiB::SIZE);

            let frame: PhysFrame<Size2MiB> = FRAME_ALLOCATOR.allocate_frame().ok_or(AllocError)?;
            frame.start_address()
        };

        let virt = phys.as_hhdm_virt();

        // SAFETY: The frame is aligned and non-null.
        let ptr = unsafe { NonNull::new_unchecked(virt.as_mut_ptr() as *mut u8) };
        Ok(NonNull::slice_from_raw_parts(ptr, size_bytes as _))
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {}
}

pub type DmaBuffer<T> = Box<T, DmaAllocator>;

#[repr(C)]
pub struct Dma<T: ?Sized>(DmaBuffer<T>);

impl<T> Dma<T> {
    /// Creates a new DMA (Direct Memory Access) buffer and is initialized
    /// with zeros.
    ///
    /// ## Examples
    /// ```rust,no_run
    /// let dma: Command = Dma::new();
    /// ```
    pub fn new() -> Self {
        let mut buffer = DmaBuffer::new_uninit_in(DmaAllocator);

        // SAFETY: Box returns a non-null and aligned pointer.
        unsafe {
            core::ptr::write_bytes(buffer.as_mut_ptr(), 0, 1);
        }

        // SAFETY: We have initialized the buffer above.
        Dma(unsafe { buffer.assume_init() })
    }

    pub fn new_uninit_slice(len: usize) -> Dma<[MaybeUninit<T>]> {
        Dma(DmaBuffer::new_uninit_slice_in(len, DmaAllocator))
    }
}

impl<T> Dma<[MaybeUninit<T>]> {
    /// ## Safety
    ///
    /// As with [`MaybeUninit::assume_init`], it is up to the caller to guarantee
    /// that the value really is in an initialized state. Calling this when the
    /// content is not yet fully initialized causes immediate undefined behavior.
    pub unsafe fn assume_init(self) -> Dma<[T]> {
        Dma(self.0.assume_init())
    }
}

impl<T: ?Sized> core::ops::Deref for Dma<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> core::ops::DerefMut for Dma<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for Dma<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Dma").field(&self.0).finish()
    }
}

impl<T: ?Sized> Dma<T> {
    pub fn addr(&self) -> PhysAddr {
        unsafe {
            let phys = (&*self.0 as *const T as *const u8) as u64;
            PhysAddr::new(phys - crate::PHYSICAL_MEMORY_OFFSET.as_u64())
        }
    }
}
