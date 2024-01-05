// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

// REFERENCE: PrimeCell UART (PL011) Technical Reference Manual

use core::fmt;
use core::fmt::Write;

use spin::Once;

use core::sync::atomic::{AtomicPtr, Ordering};

use crate::mem::paging::VirtAddr;
use crate::utils::sync::Mutex;

static SERIAL: Once<Mutex<PrimeCellSerialPort>> = Once::new();

pub struct PrimeCellSerialPort(*mut u8);

impl PrimeCellSerialPort {
    pub unsafe fn new(base: usize) -> Self {
        let base_pointer = base as *mut u8;
        Self(base_pointer)
    }

    unsafe fn write(&mut self, register: u8, value: u16) {
        core::ptr::write_volatile(self.0.offset(register as isize) as *mut u16, value);
    }

    unsafe fn read(&self, register: u8) -> u16 {
        core::ptr::read_volatile(self.0.offset(register as isize) as *mut u16)
    }

    pub fn init(mut self) -> Self {
        unsafe {
            // Enable RXE, TXE, UARTEN
            self.write(0x30, 1 << 9 | 1 << 8 | 1 << 0);

            // Disable FIFOs (use character mode instead)
            let mut flags = self.read(0x2c);
            flags &= !(1 << 4);

            // Enable IRQs
            self.write(0x38, 1 << 4);

            // Clear pending interrupts
            self.write(0x44, 0x7ff);
        }

        self
    }

    fn wait_transmit(&self) {
        // TXFE - this bit is set when the transmit holding register is empty.
        while unsafe { self.read(0x18) } & 1 << 7 != 1 << 7 {
            core::hint::spin_loop();
        }
    }

    pub fn send(&mut self, byte: u8) {
        unsafe {
            match byte {
                8 | 0x7F => {
                    self.wait_transmit();
                    self.write(0, 8);

                    self.wait_transmit();
                    self.write(0, b' ' as _);

                    self.wait_transmit();
                    self.write(0, 8);
                }

                _ => {
                    self.wait_transmit();
                    self.write(0, byte as u16);
                }
            }
        }
    }
}

unsafe impl Send for PrimeCellSerialPort {}

impl fmt::Write for PrimeCellSerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}
pub fn init() {
    // CC: https://github.com/limine-bootloader/limine/issues/215
    let mut serial = unsafe { PrimeCellSerialPort::new(0x9000000) }.init();
    SERIAL.call_once(|| Mutex::new(serial));
}

pub macro serial_print($($arg:tt)*) {
    crate::drivers::uart_pl011::_serial_print(format_args!($($arg)*))
}

pub macro serial_println {
    () => ($crate::drivers::uart_pl011::serial_print!("\n")),
    ($($arg:tt)*) => ($crate::drivers::uart_pl011::serial_print!("{}\n", format_args!($($arg)*)))
}

#[doc(hidden)]
pub fn _serial_print(args: fmt::Arguments) {
    SERIAL.get().map(|c| {
        c.lock_irq()
            .write_fmt(args)
            .expect("failed to write to serial")
    });
}
