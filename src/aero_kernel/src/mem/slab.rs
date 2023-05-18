// Copyright (C) 2021-2023 The Aero Project Developers.
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

use core::ptr::NonNull;

use intrusive_collections::UnsafeRef;

use crate::mem::paging::*;
use crate::utils::sync::Mutex;

#[repr(C)]
pub struct SlabHeader {
    /// Reference to the slab pool.
    ptr: UnsafeRef<SmallSlab>,
}

impl SlabHeader {
    /// Gets the [`SlabHeader`] from an allocated object.
    pub fn from_object<'a>(ptr: *const u8) -> &'a Self {
        assert!(!ptr.is_null());

        let ptr = (ptr as usize & !(0xfff)) as *mut SlabHeader;
        unsafe { &*ptr }
    }

    /// Returns the slab pool to which this header belongs to.
    pub fn as_slab<'a>(&'a self) -> &'a SmallSlab {
        self.ptr.as_ref()
    }
}

const_assert_eq!(core::mem::size_of::<SlabHeader>(), 8);

unsafe impl Send for SlabHeader {}
unsafe impl Sync for SlabHeader {}

/// For small slabs, the [`BufCtl`]s are stored inline.
struct BufCtl(Option<NonNull<BufCtl>>);

impl BufCtl {
    const NULL: Self = Self(None);

    /// Constructs a [`BufCtl`] from a raw pointer.
    const fn from_ptr(ptr: *mut BufCtl) -> Self {
        assert!(!ptr.is_null());

        // SAFETY: We have verified above that the pointer is non-null.
        Self(Some(unsafe { NonNull::new_unchecked(ptr) }))
    }
}

const_assert_eq!(core::mem::size_of::<BufCtl>(), 8);

unsafe impl Send for BufCtl {}
unsafe impl Sync for BufCtl {}

/// Used for allocations smaller than `1/8` of a page.
pub struct SmallSlab {
    /// Size of the slab.
    size: usize,
    first_free: Mutex<BufCtl>,
}

impl SmallSlab {
    pub const fn new(size: usize) -> Self {
        assert!(size.is_power_of_two());

        Self {
            size,
            first_free: Mutex::new(BufCtl::NULL),
        }
    }

    pub fn alloc(&self) -> *mut u8 {
        let mut first_free = self.first_free.lock_irq();

        if let Some(entry) = first_free.0 {
            *first_free = BufCtl(unsafe { entry.as_ref() }.0);
            entry.as_ptr().cast()
        } else {
            drop(first_free);

            self.expand();
            self.alloc()
        }
    }

    pub fn dealloc(&self, ptr: *mut u8) {
        assert!(!ptr.is_null());

        let mut first_free = self.first_free.lock_irq();

        let mut new_head = BufCtl::from_ptr(ptr.cast());
        new_head.0 = first_free.0;
        *first_free = new_head;
    }

    fn expand(&self) {
        let frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR.allocate_frame().expect("slab: OOM");

        let ptr = frame.start_address().as_hhdm_virt().as_mut_ptr::<u8>();
        let header_size =
            align_up(core::mem::size_of::<SlabHeader>() as u64, self.size as u64) as usize;

        let avaliable_size = Size4KiB::SIZE as usize - header_size;
        let slab_ptr = unsafe { &mut *ptr.cast::<SlabHeader>() };

        // SAFETY: We are constructing an [`UnsafeRef`] from ourselves which is a valid reference.
        slab_ptr.ptr = unsafe { UnsafeRef::from_raw(self as *const _) };

        let first_free = unsafe { ptr.add(header_size).cast() };
        *self.first_free.lock_irq() = BufCtl::from_ptr(first_free);

        // Initialize the free-list:
        //
        // For objects smaller than 1/8 of a page, A slab is built by allocating a 4KiB page,
        // placing the slab header at the end, and dividing the rest into equal-size buffers:
        //
        // ------------------------------------------------------
        // | buffer | buffer | buffer | buffer | slab header
        // ------------------------------------------------------
        //                          4KiB
        let max = (avaliable_size / self.size) - 1;
        let fact = self.size / 8;

        for i in 0..max {
            unsafe {
                let entry = first_free.add(i * fact);
                let next = first_free.add((i + 1) * fact);

                (&mut *entry).0 = Some(NonNull::new_unchecked(next));
            }
        }

        unsafe {
            let entry = &mut *first_free.add(max * fact);
            *entry = BufCtl::NULL;
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }
}
