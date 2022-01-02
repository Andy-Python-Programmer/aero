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

//! PTI (Page Table Isolation) is a feature that mitigates the Meltdown security
//! vulnerability (affecting mainly Intel's x86 CPUs) and improves kernel hardening against
//! attempts to bypass kernel address space layout randomization.
//!
//! ## Notes
//! * <https://en.wikipedia.org/wiki/Kernel_page-table_isolation>

#[allow(warnings)]
unsafe fn switch_pti_stack(old: usize, new: usize) {}

#[no_mangle]
pub unsafe extern "C" fn map_pti() {}

#[no_mangle]
pub unsafe extern "C" fn unmap_pti() {}
