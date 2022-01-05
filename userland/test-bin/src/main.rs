/*
* Copyright (C) 2021-2022 The Aero Project Developers.
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

const SOCKET_PATH: &str = "/socket.unix";

macro_rules! error {
    ($($arg:tt)*) => {
        std::print!("\x1b[1;31merror\x1b[0m: {}\n", format_args!($($arg)*))
    }
}

fn real_main() -> Result<(), AeroSyscallError> {
    let socket = sys_socket(AF_UNIX, SOCK_STREAM, 0)?;

    println!("Socket file descriptor is: {}", socket);

    let mut sock_addr = SocketAddrUnix {
        family: AF_UNIX as i16,
        path: [0; 108],
    };

    sock_addr.path[0..SOCKET_PATH.len()].copy_from_slice(SOCKET_PATH.as_bytes());

    sys_bind(
        socket,
        &SocketAddr::Unix(sock_addr),
        core::mem::size_of::<SocketAddrUnix>() as u32,
    )?;

    println!("Successfully bound to {:?}", SOCKET_PATH);

    Ok(())
}

fn main() {
    if let Err(err) = real_main() {
        error!("{:?}", err);
    }
}
