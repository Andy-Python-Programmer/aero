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

use aero_syscall::{MMapFlags, MMapProt, OpenFlags};

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
    aero_syscall::sys_open(
        "/dev/stdout",
        (OpenFlags::O_RDONLY | OpenFlags::O_RDWR).bits(),
    );
    aero_syscall::sys_write(0, ASCII_INTRO.as_bytes());
    aero_syscall::sys_write(0, b"$");
    aero_syscall::sys_fork();

    let address = aero_syscall::sys_mmap(
        0x00,
        0x100,
        MMapProt::PROT_READ | MMapProt::PROT_WRITE,
        MMapFlags::MAP_ANONYOMUS | MMapFlags::MAP_FIXED | MMapFlags::MAP_PRIVATE,
        0x00,
        0x00,
    );

    unsafe {
        *(address as *mut u8) = 32;
    }

    aero_syscall::sys_exit(0x00);
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
