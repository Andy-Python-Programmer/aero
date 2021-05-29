/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use alloc::{alloc::alloc_zeroed, sync::Arc};
use core::{alloc::Layout, any::Any, cell::UnsafeCell, mem, ptr::Unique, sync::atomic::Ordering};

use crate::apic::CPU_COUNT;

pub mod buffer;
pub mod io;
pub mod linker;
pub mod stack;

pub fn validate_slice<T>(ptr: *const T, len: usize) -> Option<&'static [T]> {
    if len == 0 {
        Some(&[])
    } else {
        Some(unsafe { core::slice::from_raw_parts(ptr, len) })
    }
}

pub fn validate_str(ptr: *const u8, len: usize) -> Option<&'static str> {
    let slice = validate_slice(ptr, len)?;

    match core::str::from_utf8(slice) {
        Ok(string) => Some(string),
        Err(_) => None,
    }
}

/// Push scratch registers.
pub macro push_scratch() {
    "
    /*
    * Push scratch registers.
    */

    push rcx
    push rdx
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r11
    "
}

/// Push preserved registers.
pub macro push_preserved() {
    "
    /*
    * Push preserved registers.
    */

    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    "
}

pub macro pop_preserved() {
    "
    /* 
    * Pop preserved registers.
    */

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    "
}

/// Pop scratch registers.
pub macro pop_scratch() {
    "
    /* 
    * Pop scratch registers.
    */

    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rax
    "
}

pub macro push_fs() {
    "
    /* 
    * Push FS segment.
    */
    
    push fs

    mov rcx, 0x18
    mov fs, cx
    "
}

pub macro pop_fs() {
    "
    /* 
    * Pop FS segment.
    */

    pop fs
    "
}

pub macro intel_asm($($code:expr,)+) {
    global_asm!(concat!($($code),+,));
}

pub macro const_unsafe($($vis:vis const $name:ident: $ty:ty = $value:expr;)*) {
    $(
        $vis const $name: $ty = unsafe { $value };
    )*
}

pub macro intel_fn {
    (
        $(#![$total:meta])*

        $(#[$outer:meta])* $fn_vis:vis extern "asm" fn $name:ident($($arg_name:ident : $arg_type:ty),*) { $($body:expr,)+ }
        $(pub extern "asm" $label_name:expr => { $($label_body:expr,)+ })*
    ) => {
        $(#[$total])*
        $crate::utils::intel_asm!(
            ".global ", stringify!($name), "\n",
            ".type ", stringify!($name), ", @function\n",
            ".section .text.", stringify!($name), ", \"ax\", @progbits\n",
            stringify!($name), ":\n",
            $($body),+,
            $(
                stringify!($label_name), ":\n",
                $($label_body),+,
            )*
            ".size ", stringify!($name), ", . - ", stringify!($name), "\n",
            ".text\n",
        );

        $(#[$total])*
        extern "C" {
            $(#[$outer])*
            $fn_vis fn $name($($arg_name : $arg_type),*);
        }
    }
}

pub trait Downcastable: Any + Send + Sync {
    fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

impl<T: Any + Send + Sync> Downcastable for T {
    fn as_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}

pub struct PerCpu<T> {
    data: UnsafeCell<Unique<T>>,
}

impl<T> PerCpu<T> {
    pub const fn new_uninit() -> PerCpu<T> {
        PerCpu::<T> {
            data: UnsafeCell::new(Unique::dangling()),
        }
    }

    pub fn new(init: fn() -> T) -> PerCpu<T> {
        let mut this = PerCpu::<T>::new_uninit();

        let cpu_count = CPU_COUNT.load(Ordering::SeqCst);

        let size = mem::size_of::<T>() * cpu_count;
        let raw = unsafe { alloc_zeroed(Layout::from_size_align_unchecked(size, 8)) as *mut T };

        unsafe {
            for i in 0..cpu_count {
                raw.add(i).write(init());
            }

            this.data = UnsafeCell::new(Unique::new_unchecked(raw));
        }

        this
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        unsafe { (&mut *self.data.get()).as_mut() }
    }

    pub fn get(&self) -> &T {
        unsafe { &*self.as_mut_ptr() }
    }
}
