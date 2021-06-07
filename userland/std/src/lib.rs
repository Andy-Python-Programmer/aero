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

//! Rust standard library implementation for Aero. This is a temporary replacement
//! until Aero's standard library gets ported into the rust compiler toolchain itself.
//!
//! ## Notes
//! Currently there is no stable or reliable to implement a custom standard library
//! for rust rather then adding it directly to the rust compiler toolchain.

#![feature(lang_items, prelude_import)]
#![no_std]

mod entry;

#[prelude_import]
pub use prelude::rust_2018::*;

pub mod prelude {
    pub mod rust_2018 {
        pub use core::prelude::v1::*;
    }
}

use core::panic::PanicInfo;

#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    loop {}
}
