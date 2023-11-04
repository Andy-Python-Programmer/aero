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

use core::fmt::{Debug, Display};
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};

use crate::interrupts::exceptions::PF_RESUME;
use crate::mem::paging::VirtAddr;
use crate::syscall::SysArg;

use super::task::user_access_ok;

/// Copy to/from a block of data from user space. Returns whether the copy was successful.
///
/// # Safety
/// The caller must ensure that:
/// * If copying to userspace, the `dest` pointer must be within the userland address space.
/// * If copying from userspace, the `dest` pointer must be valid for `size` bytes.
///
/// ## Concurrent Accesses
/// Concurrent access, *including potential data races with userspace memory*, are permitted since
/// other userspace threads or processes may modify the memory concurrently. This behavior is
/// similar to how [`std::io`] permits data races with file contents on disk.
///
/// [`std::io`]: https://doc.rust-lang.org/std/io/index.html
#[naked]
unsafe extern "C" fn copy_to_from_user(
    dest: *mut u8,
    src: *const u8,
    size: usize,

    fault_resume: *const u8,
) -> bool {
    // Registers used:
    //
    // %rdi = argument 1, `dest`
    // %rsi = argument 2, `src`
    // %rdx = argument 3, `size`
    // %rcx = argument 4, `fault_resume` (copied to %r10)
    asm!(
        // Copy `fault_resume` out of %rcx because it will be utilized by `rep movsb` latter.
        "mov r10, rcx",
        // Setup the page fault resume.
        "lea rax, 1f",
        "mov [r10], rax",
        // XXX: From this point until the fault return is reset, no function calls or stack
        // manipulations should be performed. We must ensure the ability to restore all callee
        // registers without any knowledge of the exact location within this code where a fault may
        // occur.
        //
        // Copy 8 bytes at a time and then one byte at a time for the remainder.
        "mov rcx, rdx",
        "shr rcx, 3",
        "rep movsq",
        "and edx, 7",
        "je 2f",
        "mov ecx, edx",
        "rep movsb",
        // Set return value to `true`.
        "2:",
        "mov eax, 1",
        // Reset the `fault_resume` pointer and return.
        "3:",
        "mov qword ptr [r10], 0",
        "ret",
        // `fault_resume` handler - set return value to `false` and return.
        "1:",
        "xor eax, eax",
        "jmp 3b",
        options(noreturn)
    )
}

/// Copy a structure from userspace memory. Returns whether the copy was successful.
#[must_use]
fn copy_from_user<T>(dest: &mut MaybeUninit<T>, src: *const T) -> bool {
    let fault_resume = unsafe { PF_RESUME.addr() }.as_ptr();
    let size = core::mem::size_of::<T>();

    user_access_ok(src);

    // SAFETY: We have verified that the `src` pointer is within the userland address space.
    unsafe { copy_to_from_user(dest.as_mut_ptr().cast(), src.cast(), size, fault_resume) }
}

/// Copy a structure from userspace memory. Returns whether the copy was successful.
#[must_use]
fn copy_to_user<T>(dest: *mut T, src: &T) -> bool {
    let fault_resume = unsafe { PF_RESUME.addr() }.as_ptr();
    let size = core::mem::size_of::<T>();
    let src_ptr = src as *const T;

    user_access_ok(dest);

    // SAFETY: We have verified that the `dest` pointer is within the userland address space.
    unsafe { copy_to_from_user(dest.cast(), src_ptr.cast(), size, fault_resume) }
}

/// A reference to a structure in userspace memory, which can be either read-only or read-write.
///
/// Concurrent access, *including data races to/from userspace memory*, are permitted. See the
/// documentation of [`copy_to_from_user`] for more information.
pub struct UserRef<T> {
    ptr: *mut T,
    val: T,
}

impl<T> UserRef<T> {
    pub unsafe fn new(address: VirtAddr) -> Self {
        let mut val = MaybeUninit::<T>::uninit();

        // FIXME: Return an error if the copy fails.
        assert!(copy_from_user(&mut val, address.as_ptr()));

        Self {
            ptr: address.as_mut_ptr(),
            // SAFETY: We have initialized the value via `copy_from_user` above.
            val: unsafe { val.assume_init() },
        }
    }

    pub fn take(self) -> T
    where
        T: Clone,
    {
        self.val.clone()
    }
}

impl<T> Deref for UserRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<T> DerefMut for UserRef<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

impl<T> Drop for UserRef<T> {
    fn drop(&mut self) {
        assert!(copy_to_user(self.ptr, &self.val));
    }
}

impl<T: Debug> Display for UserRef<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.val.fmt(f)
    }
}

impl<T: Debug> Debug for UserRef<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UserRef({:?})", self.val)
    }
}

impl<T: Debug> SysArg for UserRef<T> {
    fn from_usize(value: usize) -> Self {
        // TODO: SAFETY
        unsafe { Self::new(VirtAddr::new(value.try_into().unwrap())) }
    }
}
