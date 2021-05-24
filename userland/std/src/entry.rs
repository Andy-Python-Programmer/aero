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

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    extern "C" {
        /// The rustc compiler auto generates a main function for us and its definition
        /// is fixed on all platforms. We still have to use "C" ABI (as defined in rustc)
        /// because rust ABI is currently not stable *yet*.
        fn main(argc: isize, argv: *const *const u8) -> isize;
    }

    main(0, core::ptr::null());
    aero_syscall::sys_exit(0);
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, _: isize, _: *const *const u8) -> isize {
    main();

    0
}
