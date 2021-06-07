/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

//! PTI (Page Table Isolation) is a feature that mitigates the Meltdown security
//! vulnerability (affecting mainly Intel's x86 CPUs) and improves kernel hardening against
//! attempts to bypass kernel address space layout randomization.
//!
//! ## Notes
//! * <https://en.wikipedia.org/wiki/Kernel_page-table_isolation>

pub const PTI_STACK_SIZE: usize = 256;

/// The PTI CPU stack stored as a thread local.
#[thread_local]
pub static mut PTI_CPU_STACK: [u8; PTI_STACK_SIZE] = [0; PTI_STACK_SIZE];

#[allow(warnings)]
unsafe fn switch_pti_stack(old: usize, new: usize) {}

#[no_mangle]
pub unsafe extern "C" fn map_pti() {}

#[no_mangle]
pub unsafe extern "C" fn unmap_pti() {}
