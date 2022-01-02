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

use crate::utils::io;
use crate::utils::sync::Mutex;

use bitflags::bitflags;
use lazy_static::lazy_static;

const MOUSE_WAIT_TIMEOUT: usize = 100000;

lazy_static! {
    pub static ref MOUSE: Mutex<Mouse> = Mutex::new(Mouse::new());
}

bitflags! {
    /// Represents the flags currently set for the mouse.
    #[derive(Default)]
    pub struct MouseFlags: u8 {
        const LEFT_BUTTON = 0b00000001;
        const RIGHT_BUTTON = 0b00000010;
        const MIDDLE_BUTTON = 0b00000100;
        /// Whether or not the packet is valid or not.
        const ALWAYS_ONE = 0b00001000;
        /// Whether or not the x delta is negative.
        const X_SIGN = 0b00010000;
        /// Whether or not the y delta is negative.
        const Y_SIGN = 0b00100000;
        /// Whether or not the x delta overflowed.
        const X_OVERFLOW = 0b01000000;
        /// Whether or not the y delta overflowed.
        const Y_OVERFLOW = 0b10000000;
    }
}

#[derive(Debug)]
struct MouseState {
    x: i16,
    y: i16,
    flags: MouseFlags,
}

impl MouseState {
    /// Create a new mouse state
    #[inline]
    const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            flags: MouseFlags::empty(),
        }
    }

    #[inline]
    const fn is_left_button_pressed(&self) -> bool {
        self.flags.contains(MouseFlags::LEFT_BUTTON)
    }

    #[inline]
    const fn is_right_button_pressed(&self) -> bool {
        self.flags.contains(MouseFlags::RIGHT_BUTTON)
    }

    #[inline]
    const fn is_middle_button_pressed(&self) -> bool {
        self.flags.contains(MouseFlags::MIDDLE_BUTTON)
    }
}

pub struct Mouse {
    cycle: u8,
    state: MouseState,
}

impl Mouse {
    #[inline]
    const fn new() -> Self {
        Self {
            cycle: 0,
            state: MouseState::new(),
        }
    }

    fn process_mouse_packet(&mut self, packet: u8) {
        match self.cycle {
            0 => {
                let flags = MouseFlags::from_bits_truncate(packet);

                // Check if its a valid mouse packet
                if !flags.contains(MouseFlags::ALWAYS_ONE) {
                    return;
                }

                self.state.flags = flags;
            }

            1 => {
                if !self.state.flags.contains(MouseFlags::X_OVERFLOW) {
                    self.state.x = if self.state.flags.contains(MouseFlags::X_SIGN) {
                        sign_extend(packet)
                    } else {
                        packet as i16
                    };
                }
            }

            2 => {
                if !self.state.flags.contains(MouseFlags::Y_OVERFLOW) {
                    self.state.y = if self.state.flags.contains(MouseFlags::Y_SIGN) {
                        sign_extend(packet)
                    } else {
                        packet as i16
                    };
                }

                self.process_collected_packet();
            }

            _ => unreachable!(),
        }

        self.cycle = (self.cycle + 1) % 3;
    }

    fn process_collected_packet(&self) {
        if self.state.is_left_button_pressed() {
            log::debug!("Left mouse button pressed")
        }

        if self.state.is_middle_button_pressed() {
            log::debug!("Middle mouse button pressed")
        }

        if self.state.is_right_button_pressed() {
            log::debug!("Right mouse button pressed")
        }

        self.draw_mouse_pointer();
    }

    fn draw_mouse_pointer(&self) {}
}

#[inline]
fn sign_extend(packet: u8) -> i16 {
    ((packet as u16) | 0xFF00) as i16
}

/// Handle the mouse interrupt.
#[inline]
pub unsafe fn handle(data: u8) {
    MOUSE.lock().process_mouse_packet(data)
}

unsafe fn mouse_wait() {
    for _ in 0..MOUSE_WAIT_TIMEOUT {
        if io::inb(0x64 & 0b10) == 0 {
            return;
        }
    }
}

unsafe fn mouse_wait_input() {
    for _ in 0..MOUSE_WAIT_TIMEOUT {
        if io::inb(0x64) & 0b1 == 1 {
            return;
        }
    }
}

unsafe fn mouse_write(value: u8) {
    io::outb(0x64, 0xD4);
    mouse_wait();

    io::outb(0x60, value);
}

/// Initialise the PS/2 Mouse.
pub fn init() {
    unsafe {
        // Enable the auxiliary device - mouse.
        io::outb(0x64, 0xA8);
        mouse_wait();

        // Inform the keyboard controller that we want to send a command to the mouse.
        io::outb(0x64, 0x20);
        mouse_wait_input();

        let mut status = io::inb(0x60);

        status |= 0b10;
        mouse_wait();

        io::outb(0x64, 0x60);
        mouse_wait();

        io::outb(0x60, status);
        mouse_wait();

        // 0xF6 is the default settings for the mouse.
        mouse_write(0xF6);

        mouse_wait_input();
        io::inb(0x60);

        mouse_write(0xF4);

        mouse_wait_input();
        io::inb(0x60);
    }
}
