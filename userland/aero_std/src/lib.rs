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

#![feature(alloc_error_handler, lang_items, never_type, prelude_import, start)]
#![no_std]

extern crate alloc;

pub mod heap;
pub mod io;
pub mod prelude;
pub mod process;

use aero_syscall::*;
use process::Termination;

#[prelude_import]
pub use prelude::rust_2021::*;

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print_stdout(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! dbg {
    ($arg:expr) => {{
        let value = $arg;
        $crate::println!("{} = {:?}", stringify!($arg), value);
        value
    }};
}

#[doc(hidden)]
pub fn _print_stdout(args: core::fmt::Arguments) {
    use core::fmt::Write;

    let _ = io::Stdout.write_fmt(args);
}

#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);

    aero_syscall::sys_exit(42);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("Failed allocating memory with layout: {:?}", layout)
}

#[no_mangle]
unsafe extern "C" fn _start(argc: isize, argv: *const *const u8) -> ! {
    extern "C" {
        fn main(_argc: isize, _argv: *const *const u8) -> isize;
    }

    sys_exit(main(argc, argv) as usize);
}

#[lang = "start"]
fn lang_start<T: Termination + 'static>(
    main: fn() -> T,
    _argc: isize,
    _argv: *const *const u8,
) -> isize {
    main().report() as _
}

#[lang = "eh_personality"]
fn eh_personality() -> ! {
    unreachable!();
}

#[no_mangle]
#[allow(non_snake_case)]
fn _Unwind_Resume() -> ! {
    unreachable!();
}
