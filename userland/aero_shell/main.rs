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

use aero_syscall::*;
use core::panic::PanicInfo;

const ASCII_INTRO: &str = r"
_______ _______ ______ _______    _______ ______ 
(_______|_______|_____ (_______)  (_______) _____)
 _______ _____   _____) )     _    _     ( (____  
|  ___  |  ___) |  __  / |   | |  | |   | \____ \ 
| |   | | |_____| |  \ \ |___| |  | |___| |____) )
|_|   |_|_______)_|   |_\_____/    \_____(______/ 
";

fn ls(path: &str) -> Result<(), AeroSyscallError> {
    let fd = aero_syscall::sys_open(path, OpenFlags::O_DIRECTORY)?;
    let mut buffer = [0u8; 1024];

    loop {
        let size = aero_syscall::sys_getdents(fd, &mut buffer)?;

        if size == 0x00 {
            break;
        }

        let mut offset = 0x00;

        loop {
            let dptr = unsafe { buffer.as_ptr().add(offset) as *const SysDirEntry };
            let dentry = unsafe { dptr.read_unaligned() };

            let name_start = offset + core::mem::size_of::<SysDirEntry>();
            let name_end = offset + dentry.reclen;

            let name = unsafe { core::str::from_utf8_unchecked(&buffer[name_start..name_end]) };

            print!("{} ", name);

            offset += dentry.reclen;

            if offset as usize >= size {
                break;
            }
        }
    }

    sys_close(fd)?;
    println!();

    Ok(())
}

fn cat(file: &str) -> Result<(), AeroSyscallError> {
    let fd = sys_open(file, OpenFlags::O_RDONLY)?;
    let mut out = [1u8; 256];
    let length = sys_read(fd, &mut out)?;

    let contents = &unsafe { core::str::from_utf8_unchecked(&out) }[..length];
    print!("{}", contents);
    

    sys_close(fd)?;

    Ok(())
}

fn dmsg() -> Result<(), AeroSyscallError> {
    let fd = sys_open("/dev/kmsg", OpenFlags::O_RDONLY)?;
    let mut out = [1u8; 4096];
    let length = sys_read(fd, &mut out)?;

    let contents = &unsafe { core::str::from_utf8_unchecked(&out) }[..length];

    for log in contents.split("\n") {
        let mut iter = log.split(" ");

        if let Some(info) = iter.next() {
            if !info.starts_with("[") {
                continue;
            }

            let level = &info[1..][..info.len() - 2];
            let color = match level {
                "ERROR" => "\x1b[1;31m",
                "WARN" => "\x1b[1;33m",
                "INFO" => "\x1b[1;32m",
                "DEBUG" => "\x1b[1;34m",
                "TRACE" => "\x1b[1;35m",
                _ => continue,
            };

            print!("{}{}\x1b[0m", color, level);
            print!(": ");

            for rest in iter {
                print!("{} ", rest);
            }

            println!();
        }
    }

    sys_close(fd)?;

    Ok(())
}

fn init() -> Result<(), AeroSyscallError> {
    sys_open("/dev/tty", OpenFlags::O_RDONLY)?; // device: stdin
    sys_open("/dev/tty", OpenFlags::O_WRONLY)?; // device: stdout
    sys_open("/dev/tty", OpenFlags::O_WRONLY)?; // device: stderr

    println!("{}", ASCII_INTRO);
    Ok(())
}

fn main() -> Result<(), AeroSyscallError> {
    loop {
        let mut pwd_buffer = [0u8; 1024];
        sys_getcwd(&mut pwd_buffer)?;

        let pwd = unsafe { core::str::from_utf8_unchecked(&pwd_buffer) };
        let pwd = pwd.trim_matches(|c| c == '\0');

        print!("\x1b[1;32mroot@aero\x1b[0m");
        print!(":");
        print!("\x1b[1;34m{}\x1b[0m ", pwd);

        let mut buffer = [0u8; 256];
        let mut len = sys_read(0, &mut buffer)?;
        
        if len > 256 {
            len = 0;
        }

        let mut command_iter = unsafe { core::str::from_utf8_unchecked(&mut buffer) }.trim()
            [0..len]
            .split_whitespace();

        let command = command_iter.next();

        if let Some(command) = command {
            if command == "ls" {
                if let Some(dir) = command_iter.next() {
                    ls(dir)?
                } else {
                    // By default "ls" will be executed in the current directory.
                    ls(".")?
                }
            } else if command == "pwd" {
                println!("{}", pwd);
            } else if command == "mkdir" {
                if let Some(dir) = command_iter.next() {
                    sys_mkdir(dir)?;
                } else {
                    println!("mkdir: missing operand")
                }
            } else if command == "rmdir" {
                if let Some(dir) = command_iter.next() {
                    sys_rmdir(dir)?;
                } else {
                    println!("rmdir: missing operand")
                }
            } else if command == "cd" {
                if let Some(dir) = command_iter.next() {
                    sys_chdir(dir)?;
                } else {
                    // By default "cd" changes to the parent directory if no directory is specified.
                    sys_chdir("..")?;
                }
            } else if command == "cat" {
                if let Some(file) = command_iter.next() {
                    cat(file)?;
                } else {
                    println!("cat: missing operand")
                }
            } else if command == "shutdown" {
                sys_shutdown();
            } else if command == "malloc" {
                if let Some(size) = command_iter.next() {
                    let size = size.parse::<usize>().expect("malloc: invalid operand type");
                    let address = sys_mmap(
                        0,
                        size,
                        MMapProt::PROT_READ | MMapProt::PROT_WRITE,
                        MMapFlags::MAP_ANONYOMUS | MMapFlags::MAP_PRIVATE,
                        0,
                        0,
                    )?;

                    println!(
                        "malloc: allocated {}B of memory at address {:#x}",
                        size, address
                    );
                }
            } else if command == "clear" {
                print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
            } else if command == "dmsg" {
                dmsg()?;
            } else if command != "\u{0}" {
                if sys_exec(command).is_err() {
                    println!("{}: command not found", command);
                }
            }
        }
    }
}

#[no_mangle]
extern "C" fn _start() {
    init().expect("shell: failed to initialize IO file descriptors");

    loop {
        if let Err(err) = main() {
            println!("error: {:?}", err);
        }
    }
}

#[panic_handler]
extern "C" fn rust_begin_unwind(info: &PanicInfo) -> ! {
    println!("{}", info);
    sys_exit(0x01);
}

#[allow(non_snake_case)]
#[no_mangle]
extern "C" fn _Unwind_Resume() -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
