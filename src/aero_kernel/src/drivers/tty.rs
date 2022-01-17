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

use crate::fs::devfs;
use crate::fs::inode;
use crate::{aero_main, fs};

use crate::fs::inode::INodeInterface;
use crate::mem::paging::VirtAddr;
use crate::utils::sync::{BlockQueue, Mutex};

use super::keyboard::KeyCode;
use super::keyboard::KeyboardListener;

lazy_static::lazy_static! {
    static ref TTY: Arc<Tty> = Tty::new();

    static ref TERMIOS: Mutex<aero_syscall::Termios> = Mutex::new(aero_syscall::Termios {
        c_iflag: 0,
        c_oflag: aero_syscall::TermiosOFlag::empty(),
        c_cflag: aero_syscall::TermiosCFlag::empty(),
        c_lflag: aero_syscall::TermiosLFlag::ECHO | aero_syscall::TermiosLFlag::ICANON,
        c_line: 0,
        c_cc: [0; 32],
        c_ispeed: 0,
        c_ospeed: 0,
    });
}

// From the linux kernel: https://github.com/torvalds/linux/blob/master/drivers/tty/vt/defkeymap.c_shipped
const PLAIN_MAP: &[u16; 128] = &[
    0xf200, 0xf01b, 0xf031, 0xf032, 0xf033, 0xf034, 0xf035, 0xf036, 0xf037, 0xf038, 0xf039, 0xf030,
    0xf02d, 0xf03d, 0xf07f, 0xf009, 0xfb71, 0xfb77, 0xfb65, 0xfb72, 0xfb74, 0xfb79, 0xfb75, 0xfb69,
    0xfb6f, 0xfb70, 0xf05b, 0xf05d, 0xf201, 0xf702, 0xfb61, 0xfb73, 0xfb64, 0xfb66, 0xfb67, 0xfb68,
    0xfb6a, 0xfb6b, 0xfb6c, 0xf03b, 0xf027, 0xf060, 0xf700, 0xf05c, 0xfb7a, 0xfb78, 0xfb63, 0xfb76,
    0xfb62, 0xfb6e, 0xfb6d, 0xf02c, 0xf02e, 0xf02f, 0xf700, 0xf30c, 0xf703, 0xf020, 0xf207, 0xf100,
    0xf101, 0xf102, 0xf103, 0xf104, 0xf105, 0xf106, 0xf107, 0xf108, 0xf109, 0xf208, 0xf209, 0xf307,
    0xf308, 0xf309, 0xf30b, 0xf304, 0xf305, 0xf306, 0xf30a, 0xf301, 0xf302, 0xf303, 0xf300, 0xf310,
    0xf206, 0xf200, 0xf03c, 0xf10a, 0xf10b, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf30e, 0xf702, 0xf30d, 0xf01c, 0xf701, 0xf205, 0xf114, 0xf603, 0xf118, 0xf601, 0xf602, 0xf117,
    0xf600, 0xf119, 0xf115, 0xf116, 0xf11a, 0xf10c, 0xf10d, 0xf11b, 0xf11c, 0xf110, 0xf311, 0xf11d,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
];

const SHIFT_MAP: &[u16; 128] = &[
    0xf200, 0xf01b, 0xf021, 0xf040, 0xf023, 0xf024, 0xf025, 0xf05e, 0xf026, 0xf02a, 0xf028, 0xf029,
    0xf05f, 0xf02b, 0xf07f, 0xf009, 0xfb51, 0xfb57, 0xfb45, 0xfb52, 0xfb54, 0xfb59, 0xfb55, 0xfb49,
    0xfb4f, 0xfb50, 0xf07b, 0xf07d, 0xf201, 0xf702, 0xfb41, 0xfb53, 0xfb44, 0xfb46, 0xfb47, 0xfb48,
    0xfb4a, 0xfb4b, 0xfb4c, 0xf03a, 0xf022, 0xf07e, 0xf700, 0xf07c, 0xfb5a, 0xfb58, 0xfb43, 0xfb56,
    0xfb42, 0xfb4e, 0xfb4d, 0xf03c, 0xf03e, 0xf03f, 0xf700, 0xf30c, 0xf703, 0xf020, 0xf207, 0xf10a,
    0xf10b, 0xf10c, 0xf10d, 0xf10e, 0xf10f, 0xf110, 0xf111, 0xf112, 0xf113, 0xf213, 0xf203, 0xf307,
    0xf308, 0xf309, 0xf30b, 0xf304, 0xf305, 0xf306, 0xf30a, 0xf301, 0xf302, 0xf303, 0xf300, 0xf310,
    0xf206, 0xf200, 0xf03e, 0xf10a, 0xf10b, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf30e, 0xf702, 0xf30d, 0xf200, 0xf701, 0xf205, 0xf114, 0xf603, 0xf20b, 0xf601, 0xf602, 0xf117,
    0xf600, 0xf20a, 0xf115, 0xf116, 0xf11a, 0xf10c, 0xf10d, 0xf11b, 0xf11c, 0xf110, 0xf311, 0xf11d,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
];

const ALTGR_MAP: &[u16; 128] = &[
    0xf200, 0xf200, 0xf200, 0xf040, 0xf200, 0xf024, 0xf200, 0xf200, 0xf07b, 0xf05b, 0xf05d, 0xf07d,
    0xf05c, 0xf200, 0xf200, 0xf200, 0xfb71, 0xfb77, 0xf918, 0xfb72, 0xfb74, 0xfb79, 0xfb75, 0xfb69,
    0xfb6f, 0xfb70, 0xf200, 0xf07e, 0xf201, 0xf702, 0xf914, 0xfb73, 0xf917, 0xf919, 0xfb67, 0xfb68,
    0xfb6a, 0xfb6b, 0xfb6c, 0xf200, 0xf200, 0xf200, 0xf700, 0xf200, 0xfb7a, 0xfb78, 0xf916, 0xfb76,
    0xf915, 0xfb6e, 0xfb6d, 0xf200, 0xf200, 0xf200, 0xf700, 0xf30c, 0xf703, 0xf200, 0xf207, 0xf50c,
    0xf50d, 0xf50e, 0xf50f, 0xf510, 0xf511, 0xf512, 0xf513, 0xf514, 0xf515, 0xf208, 0xf202, 0xf911,
    0xf912, 0xf913, 0xf30b, 0xf90e, 0xf90f, 0xf910, 0xf30a, 0xf90b, 0xf90c, 0xf90d, 0xf90a, 0xf310,
    0xf206, 0xf200, 0xf07c, 0xf516, 0xf517, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf30e, 0xf702, 0xf30d, 0xf200, 0xf701, 0xf205, 0xf114, 0xf603, 0xf118, 0xf601, 0xf602, 0xf117,
    0xf600, 0xf119, 0xf115, 0xf116, 0xf11a, 0xf10c, 0xf10d, 0xf11b, 0xf11c, 0xf110, 0xf311, 0xf11d,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
];

const CTRL_MAP: &[u16; 128] = &[
    0xf200, 0xf200, 0xf200, 0xf000, 0xf01b, 0xf01c, 0xf01d, 0xf01e, 0xf01f, 0xf07f, 0xf200, 0xf200,
    0xf01f, 0xf200, 0xf008, 0xf200, 0xf011, 0xf017, 0xf005, 0xf012, 0xf014, 0xf019, 0xf015, 0xf009,
    0xf00f, 0xf010, 0xf01b, 0xf01d, 0xf201, 0xf702, 0xf001, 0xf013, 0xf004, 0xf006, 0xf007, 0xf008,
    0xf00a, 0xf00b, 0xf00c, 0xf200, 0xf007, 0xf000, 0xf700, 0xf01c, 0xf01a, 0xf018, 0xf003, 0xf016,
    0xf002, 0xf00e, 0xf00d, 0xf200, 0xf20e, 0xf07f, 0xf700, 0xf30c, 0xf703, 0xf000, 0xf207, 0xf100,
    0xf101, 0xf102, 0xf103, 0xf104, 0xf105, 0xf106, 0xf107, 0xf108, 0xf109, 0xf208, 0xf204, 0xf307,
    0xf308, 0xf309, 0xf30b, 0xf304, 0xf305, 0xf306, 0xf30a, 0xf301, 0xf302, 0xf303, 0xf300, 0xf310,
    0xf206, 0xf200, 0xf200, 0xf10a, 0xf10b, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf30e, 0xf702, 0xf30d, 0xf01c, 0xf701, 0xf205, 0xf114, 0xf603, 0xf118, 0xf601, 0xf602, 0xf117,
    0xf600, 0xf119, 0xf115, 0xf116, 0xf11a, 0xf10c, 0xf10d, 0xf11b, 0xf11c, 0xf110, 0xf311, 0xf11d,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
];

const SHIFT_CTRL_MAP: &[u16; 128] = &[
    0xf200, 0xf200, 0xf200, 0xf000, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf01f, 0xf200, 0xf200, 0xf200, 0xf011, 0xf017, 0xf005, 0xf012, 0xf014, 0xf019, 0xf015, 0xf009,
    0xf00f, 0xf010, 0xf200, 0xf200, 0xf201, 0xf702, 0xf001, 0xf013, 0xf004, 0xf006, 0xf007, 0xf008,
    0xf00a, 0xf00b, 0xf00c, 0xf200, 0xf200, 0xf200, 0xf700, 0xf200, 0xf01a, 0xf018, 0xf003, 0xf016,
    0xf002, 0xf00e, 0xf00d, 0xf200, 0xf200, 0xf200, 0xf700, 0xf30c, 0xf703, 0xf200, 0xf207, 0xf200,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf208, 0xf200, 0xf307,
    0xf308, 0xf309, 0xf30b, 0xf304, 0xf305, 0xf306, 0xf30a, 0xf301, 0xf302, 0xf303, 0xf300, 0xf310,
    0xf206, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf30e, 0xf702, 0xf30d, 0xf200, 0xf701, 0xf205, 0xf114, 0xf603, 0xf118, 0xf601, 0xf602, 0xf117,
    0xf600, 0xf119, 0xf115, 0xf116, 0xf11a, 0xf10c, 0xf10d, 0xf11b, 0xf11c, 0xf110, 0xf311, 0xf11d,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
];

const ALT_MAP: &[u16; 128] = &[
    0xf200, 0xf81b, 0xf831, 0xf832, 0xf833, 0xf834, 0xf835, 0xf836, 0xf837, 0xf838, 0xf839, 0xf830,
    0xf82d, 0xf83d, 0xf87f, 0xf809, 0xf871, 0xf877, 0xf865, 0xf872, 0xf874, 0xf879, 0xf875, 0xf869,
    0xf86f, 0xf870, 0xf85b, 0xf85d, 0xf80d, 0xf702, 0xf861, 0xf873, 0xf864, 0xf866, 0xf867, 0xf868,
    0xf86a, 0xf86b, 0xf86c, 0xf83b, 0xf827, 0xf860, 0xf700, 0xf85c, 0xf87a, 0xf878, 0xf863, 0xf876,
    0xf862, 0xf86e, 0xf86d, 0xf82c, 0xf82e, 0xf82f, 0xf700, 0xf30c, 0xf703, 0xf820, 0xf207, 0xf500,
    0xf501, 0xf502, 0xf503, 0xf504, 0xf505, 0xf506, 0xf507, 0xf508, 0xf509, 0xf208, 0xf209, 0xf907,
    0xf908, 0xf909, 0xf30b, 0xf904, 0xf905, 0xf906, 0xf30a, 0xf901, 0xf902, 0xf903, 0xf900, 0xf310,
    0xf206, 0xf200, 0xf83c, 0xf50a, 0xf50b, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf30e, 0xf702, 0xf30d, 0xf01c, 0xf701, 0xf205, 0xf114, 0xf603, 0xf118, 0xf210, 0xf211, 0xf117,
    0xf600, 0xf119, 0xf115, 0xf116, 0xf11a, 0xf10c, 0xf10d, 0xf11b, 0xf11c, 0xf110, 0xf311, 0xf11d,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
];

const CTRL_ALT_MAP: &[u16; 128] = &[
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf811, 0xf817, 0xf805, 0xf812, 0xf814, 0xf819, 0xf815, 0xf809,
    0xf80f, 0xf810, 0xf200, 0xf200, 0xf201, 0xf702, 0xf801, 0xf813, 0xf804, 0xf806, 0xf807, 0xf808,
    0xf80a, 0xf80b, 0xf80c, 0xf200, 0xf200, 0xf200, 0xf700, 0xf200, 0xf81a, 0xf818, 0xf803, 0xf816,
    0xf802, 0xf80e, 0xf80d, 0xf200, 0xf200, 0xf200, 0xf700, 0xf30c, 0xf703, 0xf200, 0xf207, 0xf500,
    0xf501, 0xf502, 0xf503, 0xf504, 0xf505, 0xf506, 0xf507, 0xf508, 0xf509, 0xf208, 0xf200, 0xf307,
    0xf308, 0xf309, 0xf30b, 0xf304, 0xf305, 0xf306, 0xf30a, 0xf301, 0xf302, 0xf303, 0xf300, 0xf20c,
    0xf206, 0xf200, 0xf200, 0xf50a, 0xf50b, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
    0xf30e, 0xf702, 0xf30d, 0xf200, 0xf701, 0xf205, 0xf114, 0xf603, 0xf118, 0xf601, 0xf602, 0xf117,
    0xf600, 0xf119, 0xf115, 0xf20c, 0xf11a, 0xf10c, 0xf10d, 0xf11b, 0xf11c, 0xf110, 0xf311, 0xf11d,
    0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200, 0xf200,
];

struct StdinBuffer {
    back_buffer: Vec<u8>,
    front_buffer: Vec<u8>, // more like a queue

    cursor: usize,
}

impl StdinBuffer {
    fn new() -> Self {
        Self {
            back_buffer: Vec::new(),
            front_buffer: Vec::new(),

            cursor: 0,
        }
    }

    fn swap_buffer(&mut self) {
        for c in self.back_buffer.drain(..) {
            self.front_buffer.push(c);
        }
    }

    fn is_complete(&self) -> bool {
        self.back_buffer.len() > 0 || self.front_buffer.len() > 0
    }

    fn advance_cursor(&mut self) {
        self.cursor += 1;
    }
}

struct TtyState {
    lshift: bool,
    rshift: bool,
    lctrl: bool,
    rctrl: bool,
    lalt: bool,
    altgr: bool,
    caps: bool,

    parser: vte::Parser,
}

struct Tty {
    device_id: usize,
    state: Mutex<TtyState>,
    sref: Weak<Self>,

    stdin: Mutex<StdinBuffer>,
    block_queue: BlockQueue,
}

impl Tty {
    fn new() -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            device_id: devfs::alloc_device_marker(),
            state: Mutex::new(TtyState {
                lshift: false,
                rshift: false,
                lctrl: false,
                rctrl: false,
                lalt: false,
                altgr: false,
                caps: false,

                parser: vte::Parser::new(),
            }),
            block_queue: BlockQueue::new(),
            stdin: Mutex::new(StdinBuffer::new()),
            sref: sref.clone(),
        })
    }
}

impl INodeInterface for Tty {
    fn read_at(&self, _offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        self.block_queue
            .block_on(&self.stdin, |future| future.is_complete())?;

        let mut stdin = self.stdin.lock_irq();

        // record the back buffer size before swapping
        stdin.swap_buffer();
        let back_len = stdin.front_buffer.len();

        if buffer.len() > stdin.front_buffer.len() {
            for (i, c) in stdin.front_buffer.drain(..).enumerate() {
                buffer[i] = c;
            }
        } else {
            for (i, c) in stdin.front_buffer.drain(..buffer.len()).enumerate() {
                buffer[i] = c;
            }
        }

        Ok(core::cmp::min(buffer.len(), back_len))
    }

    fn write_at(&self, _offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        let mut state = self.state.lock_irq();
        let mut performer = AnsiEscape;

        for character in buffer.iter() {
            state.parser.advance(&mut performer, *character);
        }

        Ok(buffer.len())
    }

    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            aero_syscall::TIOCGWINSZ => {
                let winsize = VirtAddr::new(arg as u64);
                let winsize = unsafe { &mut *(winsize.as_mut_ptr::<aero_syscall::WinSize>()) };

                let (rows, cols) = crate::rendy::get_rows_cols();

                winsize.ws_row = rows as u16;
                winsize.ws_col = (cols as u16) - (crate::rendy::X_PAD * 2) as u16;

                let (xpixel, ypixel) = crate::rendy::get_resolution();

                winsize.ws_xpixel = xpixel as u16;
                winsize.ws_ypixel = ypixel as u16;

                Ok(0x00)
            }

            aero_syscall::TCGETS => {
                let termios = VirtAddr::new(arg as u64);
                let termios = unsafe {
                    core::slice::from_raw_parts_mut(
                        termios.as_mut_ptr::<u8>(),
                        core::mem::size_of::<aero_syscall::Termios>(),
                    )
                };

                let lock = TERMIOS.lock_irq();
                let this = &*lock;

                let this = unsafe {
                    core::slice::from_raw_parts(
                        this as *const aero_syscall::Termios as *const u8,
                        core::mem::size_of::<aero_syscall::Termios>(),
                    )
                };

                termios.copy_from_slice(this);
                Ok(0x00)
            }

            aero_syscall::TCSETSF => {
                // Allow the output buffer to drain, discard pending input.
                let mut stdin = self.stdin.lock_irq();
                stdin.back_buffer.clear();
                stdin.cursor = 0;
                core::mem::drop(stdin);

                let termios = VirtAddr::new(arg as u64);

                let termios = unsafe {
                    core::slice::from_raw_parts(
                        termios.as_mut_ptr::<u8>(),
                        core::mem::size_of::<aero_syscall::Termios>(),
                    )
                };

                let mut lock = TERMIOS.lock_irq();
                let this = &mut *lock;

                let this = unsafe {
                    core::slice::from_raw_parts_mut(
                        this as *mut aero_syscall::Termios as *mut u8,
                        core::mem::size_of::<aero_syscall::Termios>(),
                    )
                };

                this.copy_from_slice(termios);
                Ok(0x00)
            }

            _ => Err(fs::FileSystemError::NotSupported),
        }
    }
}

impl devfs::Device for Tty {
    fn device_marker(&self) -> usize {
        self.device_id
    }

    fn device_name(&self) -> String {
        String::from("tty")
    }

    fn inode(&self) -> Arc<dyn inode::INodeInterface> {
        self.sref.upgrade().unwrap()
    }
}

impl KeyboardListener for Tty {
    fn on_key(&self, key: KeyCode, released: bool) {
        let mut state = self.state.lock();
        let termios = TERMIOS.lock_irq();

        let push_str = |k: &str| {
            // TODO: decckm
            let mut stdin = self.stdin.lock_irq();

            for each in k.bytes() {
                stdin.back_buffer.push(each);
            }
        };

        let lchar = || {
            let mut shift = state.lshift || state.rshift;
            let ctrl = state.lctrl || state.rctrl;

            if state.caps && state.caps {
                shift = !shift;
            }

            let map = match (shift, ctrl, state.lalt, state.altgr) {
                (false, false, false, false) => PLAIN_MAP,
                (true, false, false, false) => SHIFT_MAP,
                (false, true, false, false) => CTRL_MAP,
                (false, false, true, false) => ALT_MAP,
                (false, false, false, true) => ALTGR_MAP,
                (true, true, false, false) => SHIFT_CTRL_MAP,
                (false, true, true, false) => CTRL_ALT_MAP,
                _ => PLAIN_MAP,
            };

            let character =
                unsafe { core::char::from_u32_unchecked((map[key as usize] & 0xff) as _) };

            // Check if the character is actually printable printable.
            if !(0x20..0x7e).contains(&(character as u32)) {
                return;
            }

            {
                let mut stdin = self.stdin.lock_irq();

                stdin.back_buffer.push(character as u8);
                stdin.advance_cursor();
            }

            if termios.c_lflag.contains(aero_syscall::TermiosLFlag::ECHO) {
                crate::rendy::print!("{}", character);
            }
        };

        let backspace = || {
            if termios.c_lflag.contains(aero_syscall::TermiosLFlag::ICANON) {
                let mut stdin = self.stdin.lock_irq();

                if stdin.back_buffer.pop().is_some() {
                    if termios.c_lflag.contains(aero_syscall::TermiosLFlag::ECHO) {
                        crate::rendy::backspace();
                        stdin.cursor -= 1;
                    }
                }
            } else {
                push_str("\x08");
            }
        };

        if !termios.c_lflag.contains(aero_syscall::TermiosLFlag::ICANON) && !released {
            match key {
                KeyCode::KEY_BACKSPACE if !released => backspace(),
                KeyCode::KEY_CAPSLOCK if !released => state.caps = !state.caps,

                KeyCode::KEY_LEFTSHIFT => state.lshift = !released,
                KeyCode::KEY_RIGHTSHIFT => state.rshift = !released,

                KeyCode::KEY_LEFTCTRL => state.lctrl = !released,
                KeyCode::KEY_RIGHTCTRL => state.rctrl = !released,

                KeyCode::KEY_LEFTALT => state.lalt = !released,
                KeyCode::KEY_RIGHTALT => state.altgr = !released,
                KeyCode::KEY_ENTER => push_str("\n"),

                KeyCode::KEY_UP => push_str("\x1b[A"),
                KeyCode::KEY_LEFT => push_str("\x1b[D"),
                KeyCode::KEY_DOWN => push_str("\x1b[B"),
                KeyCode::KEY_RIGHT => push_str("\x1b[C"),

                _ if !released => lchar(),
                _ => {}
            }

            self.stdin.lock_irq().cursor = 0;
            self.block_queue.notify_complete();
            return;
        }

        match key {
            KeyCode::KEY_CAPSLOCK if !released => state.caps = !state.caps,
            KeyCode::KEY_ENTER | KeyCode::KEY_KPENTER if !released => {
                let mut stdin = self.stdin.lock_irq();

                stdin.back_buffer.push('\n' as u8);
                stdin.cursor = 0;

                if termios.c_lflag.contains(aero_syscall::TermiosLFlag::ECHO) {
                    crate::rendy::print!("\n");
                }

                self.block_queue.notify_complete();
            }

            KeyCode::KEY_BACKSPACE if !released => backspace(),

            KeyCode::KEY_LEFTSHIFT => state.lshift = !released,
            KeyCode::KEY_RIGHTSHIFT => state.rshift = !released,

            KeyCode::KEY_LEFTCTRL => state.lctrl = !released,
            KeyCode::KEY_RIGHTCTRL => state.rctrl = !released,

            KeyCode::KEY_LEFTALT => state.lalt = !released,
            KeyCode::KEY_RIGHTALT => state.altgr = !released,

            KeyCode::KEY_LEFT if !released => {
                let mut stdin = self.stdin.lock_irq();

                // We are at the start of the input so, we cannot shift
                // the cursor to the left anymore.
                if stdin.cursor == 0 {
                    return;
                }

                let (x, y) = crate::rendy::get_cursor_position();
                crate::rendy::set_cursor_position(x - 1, y);

                stdin.cursor -= 1;
            }

            KeyCode::KEY_RIGHT if !released => {
                let mut stdin = self.stdin.lock_irq();

                // We are at the end of the input so, we cannot shift
                // the cursor to the right anymore.
                if stdin.cursor == stdin.back_buffer.len() {
                    return;
                }

                let (x, y) = crate::rendy::get_cursor_position();
                crate::rendy::set_cursor_position(x + 1, y);

                stdin.advance_cursor();
            }

            _ if !released => lchar(),

            _ => {}
        }
    }
}

enum ParsedColor {
    Unknown,
    Foreground(u16),
    Background(u16),
}

const SGR_FOREGROUND_OFFSET_1: u16 = 30;
const SGR_BACKGROUND_OFFSET_1: u16 = 40;
const SGR_FOREGROUND_OFFSET_2: u16 = 90;
const SGR_BACKGROUND_OFFSET_2: u16 = 100;

const ANSI_COLORS: &[u32; 8] = &[
    0x00000000, // black
    0x00aa0000, // red
    0x0000aa00, // green
    0x00aa5500, // brown
    0x000000aa, // blue
    0x00aa00aa, // magenta
    0x0000aaaa, // cyan
    0x00aaaaaa, // grey
];

const ANSI_BRIGHT_COLORS: &[u32; 8] = &[
    0x00555555, // black
    0x00ff5555, // red
    0x0055ff55, // green
    0x00ffff55, // brown
    0x005555ff, // blue
    0x00ff55ff, // magenta
    0x0055ffff, // cyan
    0x00ffffff, // grey
];

fn fixed_to_rgb(fixed: u16) -> u32 {
    fixed as u32
}

struct AnsiEscape;

impl vte::Perform for AnsiEscape {
    fn print(&mut self, char: char) {
        crate::rendy::print!("{}", char);
    }

    fn execute(&mut self, byte: u8) {
        let char = byte as char;

        if char == '\n' || char == '\t' {
            crate::rendy::print!("{}", char);
        } else if char == '\r' {
            let (_, y) = crate::rendy::get_cursor_position();
            crate::rendy::set_cursor_position(0, y)
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        if ignore {
            return;
        }

        match action {
            // Moves the cursor to row `n`, column `m`. The values are
            // 1-based, and default to 1 (top left corner) if omitted.
            'H' | 'f' => {
                let mut iter = params.iter();

                let x = iter.next().unwrap_or(&[1])[0] as usize;
                let y = iter.next().unwrap_or(&[1])[0] as usize;

                let mut x = if x != 0 { x - 1 } else { x };
                let mut y = if y != 0 { y - 1 } else { y };

                let (rows, cols) = crate::rendy::get_term_info();

                // Make sure the provided coordinates are valid.
                if x >= cols {
                    x = cols - 1;
                }

                if y >= rows {
                    y = rows - 1;
                }

                // Move the cursor to the position.
                crate::rendy::set_cursor_position(x, y);
            }

            'l' | 'h' => match params.iter().next() {
                Some([25]) => {
                    // Disable the cursor if action == 'l` and enable it if action
                    // == 'h'.
                    crate::rendy::set_cursor_visibility(action == 'h')
                }

                _ => unimplemented!(),
            },

            // Clears parts of the screen.
            'J' => {
                let mut iter = params.iter();

                // If `n` is missing, it defaults to 0.
                let n = iter.next().unwrap_or(&[0])[0] as usize;

                match n {
                    // If `n` is 0 (or missing), clear from cursor to end of screen.
                    0 => {
                        let (x, y) = crate::rendy::get_cursor_position();
                        let (term_rows, term_cols) = crate::rendy::get_rows_cols();

                        let term_cols = term_cols - (crate::rendy::X_PAD * 2);

                        let rows_remaining = term_rows - (y + 1);
                        let cols_diff = term_cols - (x + 1);
                        let to_clear = rows_remaining * term_cols + cols_diff;

                        crate::rendy::set_auto_flush(false);

                        for _ in 0..to_clear {
                            crate::rendy::print!(" ");
                        }

                        crate::rendy::set_cursor_position(x, y);
                        crate::rendy::double_buffer_flush();
                        crate::rendy::set_auto_flush(true);
                    }

                    1 => unimplemented!(),

                    // If `n` is 2 or 3, clear the entire screen.
                    //
                    // TODO(Andy-Python-Programmer): When we support scrollback buffer, if `n` is
                    // 3, clear the entire scrollback buffer as well.
                    2 | 3 => crate::rendy::clear_screen(false),

                    // Unknown value, do nothing.
                    _ => unimplemented!(),
                }
            }

            'C' => {
                let mut iter = params.iter();

                // If `n` is missing, it defaults to 1.
                let mut n = iter.next().unwrap_or(&[1])[0] as usize;

                let (x, y) = crate::rendy::get_cursor_position();
                let (_, term_cols) = crate::rendy::get_rows_cols();

                let term_cols = term_cols - (crate::rendy::X_PAD * 2);

                if x + n > term_cols - 1 {
                    n = (term_cols - 1) - x;
                }

                if n == 0 {
                    n = 1;
                }

                crate::rendy::set_cursor_position(x + n, y);
            }

            'D' => {
                let mut iter = params.iter();

                // If `n` is missing, it defaults to 1.
                let mut n = iter.next().unwrap_or(&[1])[0] as usize;

                let (x, y) = crate::rendy::get_cursor_position();

                // If the cursor is already at the edge of the screen, this has no effect.
                if n > x {
                    n = x;
                }

                if n == 0 {
                    n = 1;
                }

                crate::rendy::set_cursor_position(x - n, y);
            }

            // Sets colors and style of the characters following this code.
            'm' => {
                let mut piter = params.iter();

                let mut bright = false;

                while let Some(param) = piter.next() {
                    if !param.is_empty() {
                        let p1 = param[0];

                        match p1 {
                            // Reset or normal. All attributes off.
                            0 => {
                                bright = false;
                                // TODO: Turn off dim.

                                crate::rendy::reset_default();
                            }

                            // Bold or increased intensity:
                            1 => {
                                bright = true;
                                // TODO: Turn off dim.
                            }

                            // Faint, decreased intensity, or dim.
                            2 => {
                                // TODO: Turn on dim.
                                bright = false;
                            }

                            code => {
                                let parsed_color = if code >= 30 && code <= 37 {
                                    ParsedColor::Foreground(code - SGR_FOREGROUND_OFFSET_1)
                                } else if code >= 40 && code <= 47 {
                                    ParsedColor::Background(code - SGR_BACKGROUND_OFFSET_1)
                                } else if code >= 90 && code <= 97 {
                                    ParsedColor::Foreground(code - SGR_FOREGROUND_OFFSET_2)
                                } else if code >= 100 && code <= 107 {
                                    ParsedColor::Background(code - SGR_BACKGROUND_OFFSET_2)
                                } else {
                                    ParsedColor::Unknown
                                };

                                match parsed_color {
                                    ParsedColor::Foreground(color) => {
                                        let ccode = if bright {
                                            ANSI_BRIGHT_COLORS[color as usize]
                                        } else {
                                            ANSI_COLORS[color as usize]
                                        };

                                        crate::rendy::set_text_fg(ccode);
                                    }

                                    ParsedColor::Background(color) => {
                                        let ccode = if bright {
                                            ANSI_BRIGHT_COLORS[color as usize]
                                        } else {
                                            ANSI_COLORS[color as usize]
                                        };

                                        crate::rendy::set_text_bg(ccode);
                                    }

                                    ParsedColor::Unknown => {
                                        let parse_rgb =
                                            |setter: fn(u32), piter: &mut vte::ParamsIter| {
                                                let r = piter.next().unwrap_or(&[0])[0];
                                                let g = piter.next().unwrap_or(&[0])[0];
                                                let b = piter.next().unwrap_or(&[0])[0];

                                                let color =
                                                    (r as u32) << 16 | (g as u32) << 8 | b as u32;

                                                setter(color);
                                            };

                                        let parse_fixed =
                                            |setter: fn(u32), piter: &mut vte::ParamsIter| {
                                                let fixed = piter.next().unwrap_or(&[0])[0];
                                                let color = fixed_to_rgb(fixed);

                                                setter(color);
                                            };

                                        let run_special_parser =
                                            |setter: fn(u32), piter: &mut vte::ParamsIter| {
                                                if let Some(arg_typee) = piter.next() {
                                                    let p1 = arg_typee[0];

                                                    match p1 {
                                                        // A colour number from 0 to 255, for use in 256-colour terminal
                                                        // environments.
                                                        //
                                                        // - Colours 0 to 7 are the `Black` to `White` variants respectively.
                                                        // - Colours 8 to 15 are brighter versions of the eight colours above.
                                                        // - Colours 16 to 231 contain several palettes of bright colours,
                                                        // - Colours 232 to 255 are shades of grey from black to white.
                                                        //
                                                        // [cc]: https://upload.wikimedia.org/wikipedia/commons/1/15/Xterm_256color_chart.svg
                                                        5 => parse_fixed(setter, piter),

                                                        // A 24-bit RGB color, as specified by ISO-8613-3.
                                                        2 => parse_rgb(setter, piter),

                                                        _ => (),
                                                    }
                                                }
                                            };

                                        // Background
                                        if code == 48 {
                                            run_special_parser(
                                                crate::rendy::set_text_bg,
                                                &mut piter,
                                            );
                                        }
                                        // Foreground
                                        else if code == 38 {
                                            run_special_parser(
                                                crate::rendy::set_text_fg,
                                                &mut piter,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            _ => log::debug!("unknown action: {}", action),
        }
    }
}

fn init_tty() {
    super::keyboard::register_keyboard_listener(TTY.as_ref().clone());

    devfs::install_device(TTY.clone()).expect("failed to register tty as a device");
}

crate::module_init!(init_tty);
