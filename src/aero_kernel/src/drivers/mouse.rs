// Copyright (C) 2021-2022 The Aero Project Developers.
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

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::interrupts::InterruptStack;
use crate::arch::{apic, interrupts, io};
use crate::fs::devfs::Device;
use crate::fs::inode::{INodeInterface, PollFlags, PollTable};
use crate::fs::{self, devfs};
use crate::utils::sync::{Mutex, WaitQueue};

bitflags::bitflags! {
    /// Represents the flags currently set for the mouse.
    #[derive(Default)]
    pub struct MouseFlags: u8 {
        const LEFT_BUTTON = 0b00000001;
        const RIGHT_BUTTON = 0b00000010;
        const MIDDLE_BUTTON = 0b00000100;
        const ALWAYS_ONE = 0b00001000;
        const X_SIGN = 0b00010000;
        const Y_SIGN = 0b00100000;
        const X_OVERFLOW = 0b01000000;
        const Y_OVERFLOW = 0b10000000;
    }
}

const DATA_PORT: u16 = 0x60;
const CMD_PORT: u16 = 0x64;

lazy_static::lazy_static! {
    static ref MOUSE: Arc<Mouse> = Arc::new(Mouse::new());
}

static PACKETS: Mutex<Vec<Packet>> = Mutex::new(Vec::new());

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
struct Packet {
    x: i16,
    y: i16,

    flags: MouseFlags,
}

struct Mouse {
    packet: Mutex<(Packet, usize)>,
    wq: WaitQueue,
    marker: usize,
}

impl Mouse {
    fn new() -> Mouse {
        Self {
            packet: Mutex::new((Packet::default(), 0)),
            wq: WaitQueue::new(),
            marker: devfs::alloc_device_marker(),
        }
    }

    fn process_packet(&self, packet: u8) {
        let sign_extend = |v: u8| ((v as u16) | 0xFF00) as i16;
        let mut inner = self.packet.lock_irq();

        match inner.1 {
            0 => {
                // The first packet contains the mouse flags.
                let flags = MouseFlags::from_bits_truncate(packet);
                let this = &mut inner.0;

                this.flags = flags;
            }

            1 => {
                let this = &mut inner.0;

                // The second byte contains the "delta X".
                if !this.flags.contains(MouseFlags::X_OVERFLOW) {
                    if this.flags.contains(MouseFlags::X_SIGN) {
                        this.x = sign_extend(packet);
                    } else {
                        this.x = packet as i16;
                    }
                }
            }

            2 => {
                let this = &mut inner.0;

                // The third packet contains the "delta Y".
                if !this.flags.contains(MouseFlags::Y_OVERFLOW) {
                    if this.flags.contains(MouseFlags::Y_SIGN) {
                        this.y = sign_extend(packet);
                    } else {
                        this.y = packet as i16;
                    }

                    PACKETS.lock_irq().push(*this);
                    self.wq.notify_all();
                }
            }

            _ => unreachable!(),
        }

        inner.1 = (inner.1 + 1) % 3;
    }
}

impl Device for Mouse {
    fn device_marker(&self) -> usize {
        self.marker
    }

    fn device_name(&self) -> String {
        String::from("mouse0")
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        MOUSE.clone()
    }
}

impl INodeInterface for Mouse {
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        let size = core::mem::size_of::<Packet>();
        let packet = PACKETS
            .lock_irq()
            .pop()
            .ok_or(fs::FileSystemError::WouldBlock)?;

        assert_eq!(buffer.len(), size);

        unsafe {
            *(buffer.as_mut_ptr() as *mut Packet) = packet;
        }

        Ok(size)
    }

    fn poll(&self, table: Option<&mut PollTable>) -> fs::Result<PollFlags> {
        if let Some(e) = table {
            e.insert(&MOUSE.wq)
        }

        if !PACKETS.lock_irq().is_empty() {
            Ok(PollFlags::IN)
        } else {
            Ok(PollFlags::OUT)
        }
    }
}

fn irq_handler(_stack: &mut InterruptStack) {
    let data = unsafe { io::inb(0x60) };
    MOUSE.process_packet(data);
}

pub fn ps2_mouse_init() {
    let irq_vector = interrupts::allocate_vector();
    interrupts::register_handler(irq_vector, irq_handler);

    unsafe {
        io::outb(CMD_PORT, 0xd4);
        io::outb(DATA_PORT, 0xF6);
        while io::inb(DATA_PORT) != 0xfa {}
        io::outb(CMD_PORT, 0xd4);
        io::outb(DATA_PORT, 0xf4);
        while io::inb(DATA_PORT) != 0xfa {}
        io::outb(CMD_PORT, 0xd4);
        io::outb(DATA_PORT, 0xE7);
        while io::inb(DATA_PORT) != 0xfa {}
    }

    apic::io_apic_setup_legacy_irq(12, irq_vector, 1);

    devfs::install_device(MOUSE.clone()).unwrap();
    log::trace!("ps2: initialized mouse");
}
