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

use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::RwLock;

use crate::{apic, fs};

use crate::fs::devfs::{self, Device};
use crate::fs::inode::INodeInterface;
use crate::utils::io;
use crate::utils::sync::Mutex;

pub trait KeyboardListener: Send + Sync {
    fn on_key(&self, key: KeyCode, released: bool);
}

static PS2_KEYBOARD_STATE: Mutex<Ps2KeyboardState> = Mutex::new(Ps2KeyboardState::new());
static KEYBOARD_LISTNER: RwLock<Vec<&'static dyn KeyboardListener>> = RwLock::new(Vec::new());

struct Ps2KeyboardState {
    special: bool,
    released: bool,
}

impl Ps2KeyboardState {
    #[inline]
    const fn new() -> Self {
        Self {
            special: false,
            released: false,
        }
    }

    fn flush(&self) {
        unsafe {
            while io::inb(0x64) & 1 == 1 {
                let _ = io::inb(0x60);
            }
        }
    }
}

bitflags::bitflags! {
    struct ConfigFlags: u8 {
        const FIRST_INTERRUPT = 1;
        const SECOND_INTERRUPT = 1 << 1;
        const POST_PASSED = 1 << 2;
        const CONFIG_RESERVED_3 = 1 << 3;
        const FIRST_DISABLED = 1 << 4;
        const SECOND_DISABLED = 1 << 5;
        const FIRST_TRANSLATE = 1 << 6;
        const CONFIG_RESERVED_7 = 1 << 7;
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[allow(non_camel_case_types)]
pub enum KeyCode {
    KEY_RESERVED = 0,
    KEY_ESC = 1,
    KEY_1 = 2,
    KEY_2 = 3,
    KEY_3 = 4,
    KEY_4 = 5,
    KEY_5 = 6,
    KEY_6 = 7,
    KEY_7 = 8,
    KEY_8 = 9,
    KEY_9 = 10,
    KEY_0 = 11,
    KEY_MINUS = 12,
    KEY_EQUAL = 13,
    KEY_BACKSPACE = 14,
    KEY_TAB = 15,
    KEY_Q = 16,
    KEY_W = 17,
    KEY_E = 18,
    KEY_R = 19,
    KEY_T = 20,
    KEY_Y = 21,
    KEY_U = 22,
    KEY_I = 23,
    KEY_O = 24,
    KEY_P = 25,
    KEY_LEFTBRACE = 26,
    KEY_RIGHTBRACE = 27,
    KEY_ENTER = 28,
    KEY_LEFTCTRL = 29,
    KEY_A = 30,
    KEY_S = 31,
    KEY_D = 32,
    KEY_F = 33,
    KEY_G = 34,
    KEY_H = 35,
    KEY_J = 36,
    KEY_K = 37,
    KEY_L = 38,
    KEY_SEMICOLON = 39,
    KEY_APOSTROPHE = 40,
    KEY_GRAVE = 41,
    KEY_LEFTSHIFT = 42,
    KEY_BACKSLASH = 43,
    KEY_Z = 44,
    KEY_X = 45,
    KEY_C = 46,
    KEY_V = 47,
    KEY_B = 48,
    KEY_N = 49,
    KEY_M = 50,
    KEY_COMMA = 51,
    KEY_DOT = 52,
    KEY_SLASH = 53,
    KEY_RIGHTSHIFT = 54,
    KEY_KPASTERISK = 55,
    KEY_LEFTALT = 56,
    KEY_SPACE = 57,
    KEY_CAPSLOCK = 58,
    KEY_F1 = 59,
    KEY_F2 = 60,
    KEY_F3 = 61,
    KEY_F4 = 62,
    KEY_F5 = 63,
    KEY_F6 = 64,
    KEY_F7 = 65,
    KEY_F8 = 66,
    KEY_F9 = 67,
    KEY_F10 = 68,
    KEY_NUMLOCK = 69,
    KEY_SCROLLLOCK = 70,
    KEY_KP7 = 71,
    KEY_KP8 = 72,
    KEY_KP9 = 73,
    KEY_KPMINUS = 74,
    KEY_KP4 = 75,
    KEY_KP5 = 76,
    KEY_KP6 = 77,
    KEY_KPPLUS = 78,
    KEY_KP1 = 79,
    KEY_KP2 = 80,
    KEY_KP3 = 81,
    KEY_KP0 = 82,
    KEY_KPDOT = 83,

    KEY_F11 = 87,
    KEY_F12 = 88,
    KEY_KPENTER = 96,
    KEY_RIGHTCTRL = 97,
    KEY_KPSLASH = 98,
    KEY_RIGHTALT = 100,
    KEY_HOME = 102,
    KEY_UP = 103,
    KEY_PAGEUP = 104,
    KEY_LEFT = 105,
    KEY_RIGHT = 106,
    KEY_END = 107,
    KEY_DOWN = 108,
    KEY_PAGEDOWN = 109,
    KEY_INSERT = 110,
    KEY_DELETE = 111,
    KEY_LEFTMETA = 125,
    KEY_RIGHTMETA = 126,
    KEY_COMPOSE = 127,
}

pub extern "C" fn handle(scancode: u8) {
    match scancode {
        0xE0 => PS2_KEYBOARD_STATE.lock().special = true,
        0xF0 => PS2_KEYBOARD_STATE.lock().released = true,

        _ => {
            let mut lock = PS2_KEYBOARD_STATE.lock();
            let released = lock.released;
            let keycode = if !lock.special {
                match scancode {
                    0x1c => KeyCode::KEY_A,
                    0x32 => KeyCode::KEY_B,
                    0x21 => KeyCode::KEY_C,
                    0x23 => KeyCode::KEY_D,
                    0x24 => KeyCode::KEY_E,
                    0x2b => KeyCode::KEY_F,
                    0x34 => KeyCode::KEY_G,
                    0x33 => KeyCode::KEY_H,
                    0x43 => KeyCode::KEY_I,
                    0x3b => KeyCode::KEY_J,
                    0x42 => KeyCode::KEY_K,
                    0x4b => KeyCode::KEY_L,
                    0x3a => KeyCode::KEY_M,
                    0x31 => KeyCode::KEY_N,
                    0x44 => KeyCode::KEY_O,
                    0x4d => KeyCode::KEY_P,
                    0x15 => KeyCode::KEY_Q,
                    0x2d => KeyCode::KEY_R,
                    0x1b => KeyCode::KEY_S,
                    0x2c => KeyCode::KEY_T,
                    0x3c => KeyCode::KEY_U,
                    0x2a => KeyCode::KEY_V,
                    0x1d => KeyCode::KEY_W,
                    0x22 => KeyCode::KEY_X,
                    0x35 => KeyCode::KEY_Y,
                    0x1a => KeyCode::KEY_Z,
                    0x45 => KeyCode::KEY_0,
                    0x16 => KeyCode::KEY_1,
                    0x1e => KeyCode::KEY_2,
                    0x26 => KeyCode::KEY_3,
                    0x25 => KeyCode::KEY_4,
                    0x2e => KeyCode::KEY_5,
                    0x36 => KeyCode::KEY_6,
                    0x3d => KeyCode::KEY_7,
                    0x3e => KeyCode::KEY_8,
                    0x46 => KeyCode::KEY_9,
                    0xe => KeyCode::KEY_GRAVE,
                    0x4e => KeyCode::KEY_MINUS,
                    0x55 => KeyCode::KEY_EQUAL,
                    0x5d => KeyCode::KEY_BACKSLASH,
                    0x66 => KeyCode::KEY_BACKSPACE,
                    0x29 => KeyCode::KEY_SPACE,
                    0xd => KeyCode::KEY_TAB,
                    0x58 => KeyCode::KEY_CAPSLOCK,
                    0x12 => KeyCode::KEY_LEFTSHIFT,
                    0x14 => KeyCode::KEY_LEFTCTRL,
                    0x11 => KeyCode::KEY_LEFTALT,
                    0x59 => KeyCode::KEY_RIGHTSHIFT,
                    0x5a => KeyCode::KEY_ENTER,
                    0x76 => KeyCode::KEY_ESC,
                    0x5 => KeyCode::KEY_F1,
                    0x6 => KeyCode::KEY_F2,
                    0x4 => KeyCode::KEY_F3,
                    0xc => KeyCode::KEY_F4,
                    0x3 => KeyCode::KEY_F5,
                    0xb => KeyCode::KEY_F6,
                    0x83 => KeyCode::KEY_F7,
                    0xa => KeyCode::KEY_F8,
                    0x1 => KeyCode::KEY_F9,
                    0x9 => KeyCode::KEY_F10,
                    0x78 => KeyCode::KEY_F11,
                    0x7 => KeyCode::KEY_F12,
                    0x7e => KeyCode::KEY_SCROLLLOCK,
                    0x54 => KeyCode::KEY_LEFTBRACE,
                    0x77 => KeyCode::KEY_NUMLOCK,
                    0x7c => KeyCode::KEY_KPASTERISK,
                    0x7b => KeyCode::KEY_KPMINUS,
                    0x79 => KeyCode::KEY_KPPLUS,
                    0x71 => KeyCode::KEY_KPDOT,
                    0x70 => KeyCode::KEY_KP0,
                    0x69 => KeyCode::KEY_KP1,
                    0x72 => KeyCode::KEY_KP2,
                    0x7a => KeyCode::KEY_KP3,
                    0x6b => KeyCode::KEY_KP4,
                    0x73 => KeyCode::KEY_KP5,
                    0x74 => KeyCode::KEY_KP6,
                    0x6c => KeyCode::KEY_KP7,
                    0x75 => KeyCode::KEY_KP8,
                    0x7d => KeyCode::KEY_KP9,
                    0x5b => KeyCode::KEY_RIGHTBRACE,
                    0x4c => KeyCode::KEY_SEMICOLON,
                    0x52 => KeyCode::KEY_APOSTROPHE,
                    0x41 => KeyCode::KEY_COMMA,
                    0x49 => KeyCode::KEY_DOT,
                    0x4a => KeyCode::KEY_SLASH,
                    0x61 => KeyCode::KEY_BACKSLASH,
                    _ => KeyCode::KEY_RESERVED,
                }
            } else {
                match scancode {
                    0x1f => KeyCode::KEY_LEFTMETA,
                    0x14 => KeyCode::KEY_RIGHTCTRL,
                    0x27 => KeyCode::KEY_RIGHTMETA,
                    0x11 => KeyCode::KEY_RIGHTALT,
                    0x2f => KeyCode::KEY_COMPOSE,
                    0x70 => KeyCode::KEY_INSERT,
                    0x6c => KeyCode::KEY_HOME,
                    0x7d => KeyCode::KEY_PAGEUP,
                    0x71 => KeyCode::KEY_DELETE,
                    0x69 => KeyCode::KEY_END,
                    0x7a => KeyCode::KEY_PAGEDOWN,
                    0x75 => KeyCode::KEY_UP,
                    0x6b => KeyCode::KEY_LEFT,
                    0x72 => KeyCode::KEY_DOWN,
                    0x74 => KeyCode::KEY_RIGHT,
                    0x4a => KeyCode::KEY_KPSLASH,
                    0x5a => KeyCode::KEY_KPENTER,
                    _ => KeyCode::KEY_RESERVED,
                }
            };

            lock.special = false;
            lock.released = false;

            core::mem::drop(lock);

            let listners = KEYBOARD_LISTNER.read();
            for listener in listners.iter() {
                listener.on_key(keycode, released);
            }
        }
    }
}

lazy_static::lazy_static! {
    static ref KEYBOARD: Arc<KeyboardDevice> = KeyboardDevice::new();
}

struct KeyboardDevice {
    marker: usize,
    buffer: Mutex<Vec<u8>>,
    sref: Weak<Self>,
}

impl KeyboardDevice {
    fn new() -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            marker: devfs::alloc_device_marker(),
            buffer: Mutex::new(Vec::new()),
            sref: this.clone(),
        })
    }
}

impl Device for KeyboardDevice {
    fn device_marker(&self) -> usize {
        self.marker
    }

    fn device_name(&self) -> String {
        String::from("kbd0")
    }

    fn inode(&self) -> Arc<dyn INodeInterface> {
        self.sref.upgrade().unwrap()
    }
}

impl KeyboardListener for KeyboardDevice {
    fn on_key(&self, keycode: KeyCode, released: bool) {
        if released {
            self.buffer.lock_irq().push(0x80 | keycode as u8);
        } else {
            self.buffer.lock_irq().push(keycode as u8);
        }
    }
}

impl INodeInterface for KeyboardDevice {
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        let mut sbuf = self.buffer.lock_irq();
        let drainage = core::cmp::min(buffer.len(), sbuf.len());

        for (i, byte) in sbuf.drain(..drainage).enumerate() {
            buffer[i] = byte;
        }

        Ok(drainage)
    }
}

/// This function is responsible for initializing PS2 keyboard driver.
pub fn ps2_keyboard_init() {
    let lock = PS2_KEYBOARD_STATE.lock_irq();

    unsafe {
        io::outb(0x60, 0xF5); // command: disable scanning

        if io::inb(0x60) != 0xFA {
            log::warn!("ps2: disable scanning failed, no ACK");
        }

        io::outb(0x60, 0xF4); // command: enable reporting
        lock.flush();

        if io::inb(0x60) != 0xFA {
            log::warn!("ps2: failed to enable error reporting, no ACK");
        }

        io::outb(0x64, 0x20); // command: read
        lock.flush();

        let mut config = ConfigFlags::from_bits_truncate(io::inb(0x60));

        config.remove(ConfigFlags::FIRST_DISABLED);
        config.remove(ConfigFlags::FIRST_TRANSLATE); // Use scancode set 2
        config.insert(ConfigFlags::FIRST_INTERRUPT);

        io::outb(0x64, 0x60); // command: write config
        io::outb(0x60, config.bits());

        lock.flush();
    }

    apic::io_apic_setup_legacy_irq(1, 1);

    // TODO: Move this into /dev/input instead
    // TODO: Add support for multiple keyboards
    register_keyboard_listener(KEYBOARD.as_ref().clone());
    devfs::install_device(KEYBOARD.clone()).expect("failed to install keyboard device");
}

pub extern "C" fn register_keyboard_listener(listner: &'static dyn KeyboardListener) {
    KEYBOARD_LISTNER.write().push(listner)
}

crate::module_init!(ps2_keyboard_init);
