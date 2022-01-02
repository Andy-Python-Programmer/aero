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

use super::interrupt;
use crate::time;
use crate::utils::io;
use crate::{
    apic,
    drivers::{keyboard, mouse},
};

use super::INTERRUPT_CONTROLLER;

interrupt!(
    pub unsafe fn pit_stack() {
        INTERRUPT_CONTROLLER.eoi();

        time::tick();
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
