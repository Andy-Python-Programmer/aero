// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

use crate::utils::io;

#[repr(u32)]
pub enum ExitStatus {
    Success = 0x10,
    Failure = 0x11,
}

pub fn exit_qemu(exit_status: ExitStatus) -> ! {
    // QEMU will execute `exit(((code << 1) | 1))`.
    unsafe {
        io::outl(0xf4, exit_status as u32);
    }

    // For the case that the QEMU exit attempt did not work, transition into an infinite loop.
    //
    // Calling `panic!()` here is unfeasible, since there is a good chance this function here is
    // the last expression in the `panic!()` handler itself. This prevents a possible infinite
    // loop.
    loop {
        unsafe {
            crate::arch::interrupts::halt();
        }
    }
}
