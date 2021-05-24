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

use x86_64::VirtAddr;

extern "C" {
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    #[inline(always)]
    pub fn as_ptr(&'static self) -> *const u8 {
        self as *const Self as *const u8
    }

    #[inline(always)]
    pub fn as_usize(&'static self) -> usize {
        self.as_ptr() as usize
    }

    #[inline(always)]
    pub fn virt_addr(&'static self) -> VirtAddr {
        unsafe { VirtAddr::new_unsafe(self.as_usize() as u64) }
    }
}

unsafe impl Sync for LinkerSymbol {}
unsafe impl Send for LinkerSymbol {}
