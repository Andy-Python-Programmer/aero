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

use super::{interrupt_stack, INTERRUPT_CONTROLLER};

interrupt_stack!(
    pub unsafe fn reschedule(stack: &mut InterruptStack) {
        INTERRUPT_CONTROLLER.eoi(); // Signal end of interrupt
    }
);

interrupt_stack!(
    pub unsafe fn abort(stack: &mut InterruptStack) {
        INTERRUPT_CONTROLLER.eoi(); // Signal end of interrupt
    }
);
