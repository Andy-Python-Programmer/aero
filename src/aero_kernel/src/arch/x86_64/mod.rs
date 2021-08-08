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

pub mod controlregs;
pub mod gdt;
pub mod interrupts;
pub mod task;

use crate::utils::io;

pub fn init_cpu() {
    unsafe {
        // Enable the no-execute page protection feature.
        io::wrmsr(io::IA32_EFER, io::rdmsr(io::IA32_EFER) | 1 << 11);
    }
}
