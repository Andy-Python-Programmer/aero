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

use alloc::alloc::alloc_zeroed;
use alloc::sync::Arc;
use core::alloc::Layout;
use core::any::Any;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem;
use core::ptr::Unique;

use crate::mem::paging::{align_down, ReadErr, VirtAddr};

#[cfg(target_arch = "x86_64")]
use crate::arch::apic::get_cpu_count;

#[cfg(target_arch = "aarch64")]
fn get_cpu_count() -> usize {
    1
}

pub mod bitmap;
pub mod buffer;
pub mod dma;
pub mod sync;

pub fn validate_mut_ptr<T>(ptr: *mut T) -> Result<&'static mut T, ReadErr> {
    VirtAddr::new(ptr as _).read_mut::<T>()
}

pub fn validate_ptr<T>(ptr: *const T) -> Result<&'static T, ReadErr> {
    // SAFETY: Safe to cast const pointer to mutable since the pointer is not
    //         mutated and the returned reference is immutable.
    validate_mut_ptr(ptr as *mut T).map(|e| &*e)
}

pub fn validate_slice_mut<T>(ptr: *mut T, len: usize) -> Result<&'static mut [T], ReadErr> {
    if len == 0 {
        Ok(&mut [])
    } else {
        let _ = validate_ptr(ptr)?; // ensure non-null and in-range
        let _ = validate_ptr(unsafe { ptr.add(len) })?; // ensure in-range

        // SAFETY: We have validated the pointer above.
        Ok(unsafe { core::slice::from_raw_parts_mut(ptr, len) })
    }
}

pub fn validate_slice<T>(ptr: *const T, len: usize) -> Result<&'static [T], ReadErr> {
    // SAFETY: Safe to cast const pointer to mutable since the pointer is not
    //         mutated and the returned reference is immutable.
    validate_slice_mut(ptr as *mut T, len).map(|e| &*e)
}

pub fn validate_str(ptr: *const u8, len: usize) -> Result<&'static str, ReadErr> {
    let slice = validate_slice(ptr, len)?;
    core::str::from_utf8(slice).map_err(|_| ReadErr::Null)
}

pub fn validate_array_mut<T, const COUNT: usize>(
    ptr: *mut T,
) -> Result<&'static mut [T; COUNT], ReadErr> {
    let slice = validate_slice_mut::<T>(ptr, COUNT);
    // Convert the validated slice to an array.
    //
    // SAFETY: We know that `slice` is a valid slice of `COUNT` elements.
    slice.map(|e| unsafe { &mut *(e.as_ptr() as *mut [T; COUNT]) })
}

pub trait Downcastable: Any + Send + Sync {
    fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

impl<T: Any + Send + Sync> Downcastable for T {
    fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

/// Just like [`Cell`] but with [volatile] read / write operations
///
/// [`Cell`]: https://doc.rust-lang.org/std/cell/struct.Cell.html
/// [volatile]: https://doc.rust-lang.org/std/ptr/fn.read_volatile.html
#[repr(transparent)]
pub struct VolatileCell<T> {
    value: UnsafeCell<T>,
}

impl<T: Copy> VolatileCell<T> {
    /// Returns a copy of the contained value.
    #[inline]
    pub fn get(&self) -> T {
        unsafe { core::ptr::read_volatile(self.value.get()) }
    }

    /// Sets the contained value.
    #[inline]
    pub fn set(&self, value: T) {
        unsafe { core::ptr::write_volatile(self.value.get(), value) }
    }
}

pub struct PerCpu<T> {
    data: UnsafeCell<Unique<T>>,
}

impl<T> PerCpu<T> {
    #[inline]
    pub const fn new_uninit() -> PerCpu<T> {
        PerCpu::<T> {
            data: UnsafeCell::new(Unique::dangling()),
        }
    }

    pub fn new(init: fn() -> T) -> PerCpu<T> {
        let mut this = PerCpu::<T>::new_uninit();

        let cpu_count = get_cpu_count();
        let size = mem::size_of::<T>() * cpu_count;

        let raw = unsafe { alloc_zeroed(Layout::from_size_align_unchecked(size, 8)).cast::<T>() };

        unsafe {
            for i in 0..cpu_count {
                raw.add(i).write(init());
            }

            this.data = UnsafeCell::new(Unique::new_unchecked(raw));
        }

        this
    }

    #[inline]
    pub fn as_mut_ptr(&self) -> *mut T {
        unsafe { (*self.data.get()).as_mut() }
    }

    #[inline]
    pub fn get(&self) -> &T {
        unsafe { &*self.as_mut_ptr().offset(0) }
    }

    #[inline]
    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.as_mut_ptr().offset(0) }
    }
}

pub fn slice_into_bytes<T: Sized>(slice: &[T]) -> &[u8] {
    let data = slice.as_ptr().cast::<u8>();
    let size = core::mem::size_of_val(slice);

    unsafe { core::slice::from_raw_parts(data, size) }
}

pub struct StackHelper<'a> {
    ptr: &'a mut u64,
}

impl<'a> StackHelper<'a> {
    pub fn new(ptr: &'a mut u64) -> StackHelper<'a> {
        StackHelper::<'a> { ptr }
    }

    pub fn skip_by(&mut self, by: u64) {
        *self.ptr -= by;
    }

    pub unsafe fn offset<T: Sized>(&mut self) -> &mut T {
        self.skip_by(core::mem::size_of::<T>() as u64);

        &mut *(*self.ptr as *mut T)
    }

    pub fn top(&self) -> u64 {
        *self.ptr
    }

    pub unsafe fn write_slice<T: Sized>(&mut self, slice: &[T]) {
        self.write_bytes(slice_into_bytes(slice));
    }

    pub fn align_down(&mut self) {
        *self.ptr = align_down(*self.ptr, 16);
    }

    pub unsafe fn write<T: Sized>(&mut self, value: T) {
        self.skip_by(core::mem::size_of::<T>() as u64);

        (*self.ptr as *mut T).write(value);
    }

    pub unsafe fn write_bytes(&mut self, bytes: &[u8]) {
        self.skip_by(bytes.len() as u64);

        (*self.ptr as *mut u8).copy_from(bytes.as_ptr(), bytes.len());
    }

    pub fn get_by(&mut self, by: u64) {
        *self.ptr += by;
    }

    pub unsafe fn get<'b, T: Sized>(&mut self) -> &'b mut T {
        let x = &mut *(*self.ptr as *mut T);

        self.get_by(core::mem::size_of::<T>() as u64);
        x
    }
}

#[macro_export]
macro_rules! extern_sym {
    ($sym:ident) => {{
        extern "C" {
            static $sym: ::core::ffi::c_void;
        }

        // SAFETY: The value is not accessed, we only take its address. The `addr_of!()` ensures
        // that no intermediate references is created.
        unsafe { ::core::ptr::addr_of!($sym) }
    }};
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct IncompleteArrayField<T>(PhantomData<T>, [T; 0]);

impl<T> IncompleteArrayField<T> {
    #[inline]
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        self as *mut _ as *mut T
    }

    #[inline]
    pub unsafe fn as_mut_slice(&mut self, len: usize) -> &mut [T] {
        core::slice::from_raw_parts_mut(self.as_mut_ptr(), len)
    }
}
