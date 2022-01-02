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

#![feature(lang_items, start)]
#![no_std]

use aero_syscall::*;

#[no_mangle]
unsafe extern "C" fn _start(argc: isize, argv: *const *const u8) -> ! {
    extern "C" {
        fn main(_: isize, _: *const *const u8) -> isize;
    }

    let exit_code = main(argc, argv);

    sys_exit(exit_code as usize);
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, _: isize, _: *const *const u8) -> isize {
    main();

    0
}

#[allow(non_snake_case)]
#[no_mangle]
extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
