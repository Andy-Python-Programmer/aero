//! Rust standard library implementation for Aero. This is a temporary replacement
//! until Aero's standard library gets ported into the rust compiler toolchain itself.

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
