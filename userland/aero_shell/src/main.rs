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

use aero_syscall::*;

const HOSTNAME: &str = "root@aero";
const MAGENTA_FG: &str = "\x1b[1;35m";
const RESET: &str = "\x1b[0m";
const UWUFETCH_LOGO: &str = r#"
    ,---,
   '  .' \
  /  ;    '.
 :  :       \
 :  |   /\   \
 |  :  ' ;.   :
 |  |  ;/  \   \
 '  :  | \  \ ,'
 |  |  '  '--'
 |  :  :
 |  | ,'
 `--''
"#;

fn repl(history: &mut Vec<String>) -> Result<(), AeroSyscallError> {
    let mut pwd_buffer = [0; 1024];
    let mut cmd_buffer = [0; 1024];

    let pwd_length = sys_getcwd(&mut pwd_buffer)?;
    let pwd = unsafe { core::str::from_utf8_unchecked(&pwd_buffer[0..pwd_length]) };

    let mut hostname_split = HOSTNAME.splitn(2, '@');
    let username = hostname_split.next().unwrap_or("root");
    let hostname = hostname_split.next().unwrap_or("aero");

    print!(
        "\x1b[1;32m{}@{}\x1b[0m:\x1b[1;34m{}\x1b[0m ",
        username, hostname, pwd
    );

    let cmd_length = sys_read(0, &mut cmd_buffer)?;
    let cmd_string = unsafe { core::str::from_utf8_unchecked(&cmd_buffer[0..cmd_length]).trim() };

    let mut args = cmd_string.split_whitespace();

    if let Some(cmd) = args.next() {
        // history.push(cmd_string.to_string());

        match cmd {
            "ls" => list_directory(args.next().unwrap_or("."))?,
            "pwd" => println!("{}", pwd),
            "cd" => {
                sys_chdir(args.next().unwrap_or(".."))?;
            }
            "mkdir" => match args.next() {
                Some(path) => {
                    sys_mkdir(path)?;
                }
                None => println!("mkdir: missing operand"),
            },
            "rmdir" => match args.next() {
                Some(path) => {
                    sys_rmdir(path)?;
                }
                None => println!("rmdir: missing operand"),
            },
            "exit" => match args.next() {
                Some(status) => match status.parse::<usize>() {
                    Ok(exit_code) => sys_exit(exit_code),
                    Err(_) => println!("exit: invalid operand"),
                },
                None => sys_exit(0),
            },
            "cat" => cat_file(args.next())?,
            "clear" => print!("{esc}[2J{esc}[1;1H", esc = 27 as char),
            "dmsg" => print_kernel_log()?,
            "uwufetch" => uwufetch()?,
            "uname" => uname()?,
            "history" => {
                for entry in history.iter() {
                    println!("{}", entry);
                }
            }
            _ => {
                let child = sys_fork()?;

                if child == 0 {
                    if sys_exec(cmd).is_err() {
                        println!("{}: command not found", cmd);
                    }
                } else {
                    // Wait for the child
                    let mut status = 0;
                    sys_waitpid(child, &mut status, 0)?;

                    println!("back to parent. say vola");
                }
            }
        }
    }

    Ok(())
}

fn list_directory(path: &str) -> Result<(), AeroSyscallError> {
    let dir_fd = sys_open(path, OpenFlags::O_DIRECTORY)?;

    loop {
        let mut offset = 0;
        let mut dir_ents_buffer = [0; 1024];

        let size = sys_getdents(dir_fd, &mut dir_ents_buffer)?;

        if size == 0 {
            break;
        }

        while offset < size {
            let dir_entry =
                unsafe { &*(dir_ents_buffer.as_ptr().add(offset) as *const SysDirEntry) };

            let name_start = offset + core::mem::size_of::<SysDirEntry>();
            let name_end = offset + dir_entry.reclen;

            let name =
                unsafe { core::str::from_utf8_unchecked(&dir_ents_buffer[name_start..name_end]) };

            offset += dir_entry.reclen;

            print!("{} ", name);
        }
    }

    println!();

    Ok(())
}

fn cat_file(path: Option<&str>) -> Result<(), AeroSyscallError> {
    // On the `None` arm we default to 0 to take input from stdin.
    // This is the behaviour of `cat` that comes with any modern Linux distro.
    let fd = match path {
        Some(path) => sys_open(path, OpenFlags::O_RDONLY)?,
        None => 0,
    };

    let mut buffer = [0; 1024];

    loop {
        let length = sys_read(fd, &mut buffer)?;

        if length == 0 {
            break;
        }

        let contents = unsafe { core::str::from_utf8_unchecked(&buffer[0..length]) };

        print!("{}", contents);
    }

    if fd != 0 {
        sys_close(fd)?;
    }

    Ok(())
}

fn print_kernel_log() -> Result<(), AeroSyscallError> {
    // dmsg is just a wrapper around `cat /dev/kmsg`
    // TODO: Add colored output back :^)

    cat_file(Some("/dev/kmsg"))
}

fn uwufetch() -> Result<(), AeroSyscallError> {
    let print_prefix = |prefix| {
        print!("{}{}{}: ", MAGENTA_FG, prefix, RESET);
    };

    for (i, line) in UWUFETCH_LOGO.lines().skip(1).enumerate() {
        print!(" {}{:<19}{}", MAGENTA_FG, line, RESET);

        if i == 1 {
            println!("{}", HOSTNAME);
        } else if i == 2 {
            println!("{}", "-".repeat(HOSTNAME.len()));
        } else if i == 3 {
            print_prefix("OS");
            println!("Aero");
        } else if i == 4 {
            let mut uname_info = Utsname::default();

            sys_uname(&mut uname_info)?;

            print_prefix("Kernel");
            println!(
                "{} {} ({})",
                uname_info.name(),
                uname_info.version(),
                uname_info.machine()
            );
        } else {
            println!();
        }
    }

    Ok(())
}

fn uname() -> Result<(), AeroSyscallError> {
    let mut uname_info = Utsname::default();

    sys_uname(&mut uname_info)?;

    println!(
        "{} {} {} {} {}",
        uname_info.name(),
        uname_info.nodename(),
        uname_info.release(),
        uname_info.version(),
        uname_info.machine()
    );

    Ok(())
}

fn main() {
    sys_open("/dev/tty", OpenFlags::O_RDONLY).expect("Failed to open stdin");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stdout");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stderr");

    let mut history = vec![];

    loop {
        if let Err(error) = repl(&mut history) {
            println!("error: {:?}", error);
        }
    }
}
