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

use x86_64::instructions::tlb;

use super::interrupt_stack;
use crate::{apic, time};

interrupt_stack!(
    pub unsafe fn wakeup(stack: &mut InterruptStack) {
        apic::get_local_apic().eoi();
    }
);

interrupt_stack!(
    pub unsafe fn tlb(stack: &mut InterruptStack) {
        apic::get_local_apic().eoi();

        tlb::flush_all();
    }
);

interrupt_stack!(
    pub unsafe fn pit(stack: &mut InterruptStack) {
        apic::get_local_apic().eoi();

        time::PIT.tick();
    }
);
