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

#[cfg(feature = "ci")]
use crate::emu;

pub struct Test {
    pub test_fn: fn(),
    pub path: &'static str,
}

pub(crate) fn test_runner(tests: &[&Test]) {
    crate::rendy::clear_screen(true);
    crate::logger::set_rendy_debug(true);

    log::info!("running {} tests", tests.len());

    let mut passed = 0usize;

    for test in tests {
        (test.test_fn)();
        log::info!("test {} ... ok", test.path);

        passed += 1;
    }

    log::info!("");
    log::info!(
        "test result: ok. {} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out",
        passed
    );

    #[cfg(feature = "ci")]
    emu::exit_qemu(emu::ExitStatus::Success);

    loop {
        unsafe {
            crate::arch::interrupts::halt();
        }
    }
}
