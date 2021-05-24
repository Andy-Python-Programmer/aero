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

use super::interrupt;
use crate::utils::io;
use crate::{
    apic,
    drivers::{keyboard, mouse},
};
use crate::{
    arch::interrupts::{end_pic1, end_pic2},
    time,
};

interrupt!(
    pub unsafe fn lapic_error(stack: &mut InterruptStack) {
        log::error!("Local apic error");
        log::error!("ESR={:#0x}", apic::get_local_apic().get_esr());

        apic::get_local_apic().eoi();

        loop {}
    }
);

interrupt!(
    pub unsafe fn pit(stack: &mut InterruptStack) {
        time::PIT.tick();

        end_pic1();
    }
);

interrupt!(
    pub unsafe fn keyboard(stack: &mut InterruptStack) {
        let scancode = io::inb(0x60);

        keyboard::handle(scancode);
        end_pic1();
    }
);

interrupt!(
    pub unsafe fn mouse(stack: &mut InterruptStack) {
        let data = io::inb(0x60);

        mouse::handle(data);
        end_pic2();
    }
);
