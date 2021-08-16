/*
 * Copyright (C) 2021 The Aero Project Developers.
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

#![feature(lang_items, asm)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

use aero_syscall::OpenFlags;

const ASCII_INTRO: &str = r"
_______ _______ ______ _______    _______ ______ 
(_______|_______|_____ (_______)  (_______) _____)
 _______ _____   _____) )     _    _     ( (____  
|  ___  |  ___) |  __  / |   | |  | |   | \____ \ 
| |   | | |_____| |  \ \ |___| |  | |___| |____) )
|_|   |_|_______)_|   |_\_____/    \_____(______/ 
";

#[no_mangle]
extern "C" fn _start() {
    aero_syscall::sys_open("/dev/tty", OpenFlags::O_RDONLY); // device: stdin
    aero_syscall::sys_open("/dev/tty", OpenFlags::O_WRONLY); // device: stdout
    aero_syscall::sys_open("/dev/tty", OpenFlags::O_WRONLY); // device: stderr

    aero_syscall::sys_write(1, ASCII_INTRO.as_bytes());
    aero_syscall::sys_write(1, b"\n");

    loop {
        aero_syscall::sys_write(1, b"root@aero:/ ");

        let mut buffer = [0u8; 256];
        aero_syscall::sys_read(0, &mut buffer);

        let command = unsafe { core::str::from_utf8_unchecked(&mut buffer) };

        if command.starts_with("echo") {
            aero_syscall::sys_write(1, b"what should I echo :^)\n");
        } else if command.starts_with("\n") {
        } else {
            aero_syscall::sys_write(1, b"invalid command ;)\n");
        }
    }
}

#[panic_handler]
extern "C" fn rust_begin_unwind(_info: &PanicInfo) -> ! {
    aero_syscall::sys_exit(0x01);
}

#[allow(non_snake_case)]
#[no_mangle]
extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
