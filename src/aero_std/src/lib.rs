#![feature(prelude_import)]
#![no_std]

extern crate aero_rt;

pub mod io;
pub mod prelude;

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
