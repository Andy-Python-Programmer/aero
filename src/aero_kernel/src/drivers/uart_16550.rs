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

use crate::utils::io;
use crate::utils::sync::Mutex;

static COM_1: Once<Mutex<SerialPort>> = Once::new();

bitflags::bitflags! {
    pub struct InterruptEnable: u8 {
        const RECEIVED = 1;
        const SENT = 1 << 1;
        const ERRORED = 1 << 2;
        const STATUS_CHANGE = 1 << 3;
    }
}

bitflags::bitflags! {
    pub struct LineStatus: u8 {
        const INPUT_FULL = 1;
        const OUTPUT_EMPTY = 1 << 5;
    }
}

/// An interface to a serial port that allows sending out individual bytes.
#[repr(transparent)]
pub struct SerialPort(u16);

impl SerialPort {
    #[inline(always)]
    pub const fn new(port: u16) -> Self {
        Self(port)
    }

    /// Initialize the serial port.
    pub unsafe fn init(self) -> Self {
        // Disable interrupts.
        io::outb(self.0 + 1, 0x00);

        // Enable DLAB.
        io::outb(self.0 + 3, 0x80);

        // Set maximum speed to 38400 bps by configuring DLL and DLM.
        io::outb(self.0, 0x03);
        io::outb(self.0 + 1, 0x00);

        // Disable DLAB and set data word length to 8 bits.
        io::outb(self.0 + 3, 0x03);

        // Enable FIFO, clear TX/RX queues and set interrupt watermark at 14 bytes.
        io::outb(self.0 + 2, 0xC7);

        // Mark data terminal ready, signal request to send and enable auxilliary
        // output #2 (used as interrupt line for CPU).
        io::outb(self.0 + 4, 0x0B);

        // Enable interrupts.
        io::outb(self.0 + 1, 0x01);

        self
    }

    pub fn line_status(&self) -> LineStatus {
        unsafe {
            let status = io::inb(self.0 + 5);

            LineStatus::from_bits_truncate(status)
        }
    }

    fn wait_for_line_status(&self, line_status: LineStatus) {
        while !self.line_status().contains(line_status) {
            core::hint::spin_loop()
        }
    }

    pub fn send_byte(&mut self, byte: u8) {
        unsafe {
            match byte {
                8 | 0x7F => {
                    self.wait_for_line_status(LineStatus::OUTPUT_EMPTY);
                    io::outb(self.0, 8);

                    self.wait_for_line_status(LineStatus::OUTPUT_EMPTY);
                    io::outb(self.0, b' ');

                    self.wait_for_line_status(LineStatus::OUTPUT_EMPTY);
                    io::outb(self.0, 8);
                }
                _ => {
                    self.wait_for_line_status(LineStatus::OUTPUT_EMPTY);
                    io::outb(self.0, byte)
                }
            }
        }
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

/// Initialize the serial ports if avaliable.
pub fn init() {
    unsafe {
        let com_1 = SerialPort::new(0x3F8).init();

        COM_1.call_once(move || Mutex::new(com_1));
    }
}

pub macro serial_print($($arg:tt)*) {
    crate::drivers::uart_16550::_serial_print(format_args!($($arg)*))
}

pub macro serial_println {
    () => ($crate::drivers::uart_16550::serial_print!("\n")),
    ($($arg:tt)*) => ($crate::drivers::uart_16550::serial_print!("{}\n", format_args!($($arg)*)))
}

#[doc(hidden)]
pub fn _serial_print(args: fmt::Arguments) {
    COM_1.get().map(|c| {
        //
        c.lock_irq()
            .write_fmt(args)
            .expect("failed to write to COM1")
    });
}
