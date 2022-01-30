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

fn init_main() -> Result<(), AeroSyscallError> {
    let shell_pid = sys_fork()?;

    if shell_pid == 0 {
        sys_exec("/bin/aero_shell", &["/bin/aero_shell"], &[])?;
    } else {
        let mut shell_exit_code = 0;

        sys_waitpid(shell_pid, &mut shell_exit_code, 0)?;
        sys_exit(shell_exit_code as usize);
    }

    Ok(())
}

fn main() -> Result<(), AeroSyscallError> {
    sys_open("/dev/tty", OpenFlags::O_RDONLY)?;
    sys_open("/dev/tty", OpenFlags::O_WRONLY)?;
    sys_open("/dev/tty", OpenFlags::O_WRONLY)?;

    init_main()
}
