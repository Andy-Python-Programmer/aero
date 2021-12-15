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

mod consts;

use aero_syscall::*;

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

fn uwufetch() -> Result<(), AeroSyscallError> {
    const BLACK: &str = "\x1b[1;40m";
    const RED: &str = "\x1b[1;41m";
    const GREEN: &str = "\x1b[1;42m";
    const YELLOW: &str = "\x1b[1;43m";
    const BLUE: &str = "\x1b[1;44m";
    const MAGENTA: &str = "\x1b[1;45m";
    const CYAN: &str = "\x1b[1;46m";
    const WHITE: &str = "\x1b[1;47m";

    const BLACK_DEFAULT: &str = "\x1b[0;40m";
    const RED_DEFAULT: &str = "\x1b[0;41m";
    const GREEN_DEFAULT: &str = "\x1b[0;42m";
    const YELLOW_DEFAULT: &str = "\x1b[0;43m";
    const BLUE_DEFAULT: &str = "\x1b[0;44m";
    const MAGENTA_DEFAULT: &str = "\x1b[0;45m";
    const CYAN_DEFAULT: &str = "\x1b[0;46m";
    const WHITE_DEFAULT: &str = "\x1b[0;47m";

    const MAGENTA_FG: &str = "\x1b[1;35m";
    const RESET: &str = "\x1b[0m";

    let mut struc = Utsname::default();
    sys_uname(&mut struc)?;

    for (i, line) in consts::UWU_FETCH.lines().enumerate() {
        if i < 3 {
            println!("{}", line);
        } else if i == 4 {
            println!(
                "{}  {}OS{}: {} ({})",
                line,
                MAGENTA_FG,
                RESET,
                struc.name(),
                struc.machine()
            );
        } else if i == 6 {
            println!(
                "{}  {}  {}  {}  {}  {}  {}  {}  {}",
                line, BLACK, RED, GREEN, YELLOW, BLUE, MAGENTA, CYAN, WHITE
            );
        } else if i == 7 {
            println!(
                "{}  {}  {}  {}  {}  {}  {}  {}  {}",
                line,
                BLACK_DEFAULT,
                RED_DEFAULT,
                GREEN_DEFAULT,
                YELLOW_DEFAULT,
                BLUE_DEFAULT,
                MAGENTA_DEFAULT,
                CYAN_DEFAULT,
                WHITE_DEFAULT
            );
        } else {
            println!("{}", line);
        }
    }

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

fn shell() -> Result<(), AeroSyscallError> {
    loop {
        let mut pwd_buffer = [0u8; 1024];
        sys_getcwd(&mut pwd_buffer)?;

        let pwd = unsafe { core::str::from_utf8_unchecked(&pwd_buffer) };
        let pwd = pwd.trim_matches(|c| c == '\0');

        print!("\x1b[1;32mroot@aero\x1b[0m");
        print!(":");
        print!("\x1b[1;34m{}\x1b[0m ", pwd);

        let mut buffer = [0u8; 256];
        let len = sys_read(0, &mut buffer)?;

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
            } else if command == "uwufetch" {
                uwufetch()?;
            } else if command == "uname" {
                let mut struc = Utsname::default();

                sys_uname(&mut struc)?;
                println!(
                    "{} {} {} {} {}",
                    struc.name(),
                    struc.nodename(),
                    struc.release(),
                    struc.version(),
                    struc.machine()
                );
            } else if command != "\u{0}" {
                let pid = sys_fork()?;

                if pid == 0 {
                    if sys_exec(command).is_err() {
                        println!("{}: command not found", command);
                    }
                } else {
                    // Wait for the child
                }
            }
        }
    }
}

fn main() {
    println!("{}", ASCII_INTRO);

    if let Err(error) = shell() {
        println!("error: {:?}", error);
    }
}
