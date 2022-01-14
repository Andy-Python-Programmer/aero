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

struct Test<'a> {
    path: &'a str,
    func: fn(),
}

static TEST_FUNCTIONS: &[&'static Test<'static>] = &[&clone_process];

fn main() {
    sys_open("/dev/tty", OpenFlags::O_RDONLY).expect("Failed to open stdin");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stdout");
    sys_open("/dev/tty", OpenFlags::O_WRONLY).expect("Failed to open stderr");

    println!("Running userland tests...");

    for test_function in TEST_FUNCTIONS {
        (test_function.func)();
        println!("test {} ... ok", test_function.path);
    }
}

#[utest_proc::test]
fn clone_process() {}
