#![feature(lang_items, start)]
#![no_std]

use aero_syscall::*;

#[no_mangle]
unsafe extern "C" fn _start(argc: isize, argv: *const *const u8) -> ! {
    extern "C" {
        fn main(_: isize, _: *const *const u8) -> isize;
    }

    sys_open("/dev/tty", OpenFlags::O_RDONLY).expect("Failed to open stdin");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stdout");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stderr");

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
