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

use core::fmt;
use core::fmt::Write;

use spin::Once;

use crate::utils::sync::Mutex;

static SERIAL: Once<Mutex<SerialPort>> = Once::new();

#[repr(transparent)]
pub struct SerialPort(u16);

impl SerialPort {
    #[inline(always)]
    pub const fn new(port: u16) -> Self {
        Self(port)
    }

    pub fn send_byte(&mut self, byte: u8) {
        unimplemented!()
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        for byte in string.bytes() {
            self.send_byte(byte);
        }

        Ok(())
    }
}

pub fn init() {
    unimplemented!()
}

pub macro serial_print($($arg:tt)*) {
    crate::drivers::uart_mmio32::_serial_print(format_args!($($arg)*))
}

pub macro serial_println {
    () => ($crate::drivers::uart_mmio32::serial_print!("\n")),
    ($($arg:tt)*) => ($crate::drivers::uart_mmio32::serial_print!("{}\n", format_args!($($arg)*)))
}

#[doc(hidden)]
pub fn _serial_print(args: fmt::Arguments) {
    SERIAL.get().map(|c| {
        c.lock_irq()
            .write_fmt(args)
            .expect("failed to write to serial")
    });
}
