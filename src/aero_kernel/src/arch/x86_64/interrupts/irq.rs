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

use super::{interrupt, interrupt_stack};
use crate::time;
use crate::utils::io;
use crate::{
    apic,
    drivers::{keyboard, mouse},
};

use super::INTERRUPT_CONTROLLER;

interrupt_stack!(
    pub unsafe fn pit_stack(stack: &mut InterruptStack) {
        time::PIT.tick();

        INTERRUPT_CONTROLLER.eoi();
    }
);

interrupt!(
    pub unsafe fn lapic_error() {
        log::error!("Local apic error");
        log::error!("ESR={:#0x}", apic::get_local_apic().get_esr());

        apic::get_local_apic().eoi();
    }
);

interrupt!(
    pub unsafe fn keyboard() {
        let scancode = io::inb(0x60);

        keyboard::handle(scancode);
        INTERRUPT_CONTROLLER.eoi();
    }
);

interrupt!(
    pub unsafe fn mouse() {
        let data = io::inb(0x60);

        mouse::handle(data);
        INTERRUPT_CONTROLLER.eoi();
    }
);
