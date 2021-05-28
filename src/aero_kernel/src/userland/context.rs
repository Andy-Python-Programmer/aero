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

use x86_64::{PhysAddr, VirtAddr};

#[repr(C)]
pub(super) struct Context {
    pub cr3: u64,
    pub rsp: u64,
    pub rflags: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub rip: u64,
}

impl Context {
    pub fn new() -> Self {
        Self {
            cr3: 0x00,
            rsp: 0x00,
            rflags: 0x00,
            r15: 0x00,
            r14: 0x00,
            r13: 0x00,
            r12: 0x00,
            rbp: 0x00,
            rbx: 0x00,
            rip: 0x00,
        }
    }

    pub fn set_stack_top(&mut self, stack_top: VirtAddr) {
        self.rsp = stack_top.as_u64();
    }

    pub fn set_instruction_ptr(&mut self, func: VirtAddr) {
        self.rip = func.as_u64();
    }

    pub fn set_page_table(&mut self, page_table: PhysAddr) {
        self.cr3 = page_table.as_u64();
    }

    pub fn get_stack_top(&self) -> VirtAddr {
        unsafe { VirtAddr::new_unsafe(self.rsp) }
    }

    pub fn get_instruction_ptr(&self) -> VirtAddr {
        unsafe { VirtAddr::new_unsafe(self.rip) }
    }
}
