// Copyright (C) 2021-2023 The Aero Project Developers.
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

// `/dev/ctty`: controlling terminal of the process
// `/dev/vtty[0-9]`: virtual terminals

mod ctty;

use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use crate::fs::inode::{self, PollFlags, PollTable};
use crate::fs::{devfs, FileSystemError};
use crate::{fs, rendy};

use crate::fs::inode::INodeInterface;
use crate::mem::paging::VirtAddr;
use crate::userland::scheduler;
use crate::userland::task::Task;
use crate::userland::terminal::TerminalDevice;
use crate::utils::sync::{Mutex, WaitQueue};

#[cfg(target_arch = "x86_64")]
use super::keyboard::KeyCode;
#[cfg(target_arch = "x86_64")]
use super::keyboard::KeyboardListener;

lazy_static::lazy_static! {
    static ref TTY: Arc<Tty> = Tty::new();

    static ref TERMIOS: Mutex<aero_syscall::Termios> = Mutex::new(aero_syscall::Termios {
        c_iflag: aero_syscall::TermiosIFlag::empty(),
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
        !self.back_buffer.is_empty() || !self.front_buffer.is_empty()
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
}

struct Tty {
    device_id: usize,
    state: Mutex<TtyState>,
    sref: Weak<Self>,

    stdin: Mutex<StdinBuffer>,
    block_queue: WaitQueue,

    connected: AtomicUsize,
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
            }),
            block_queue: WaitQueue::new(),
            stdin: Mutex::new(StdinBuffer::new()),
            connected: AtomicUsize::new(0),
            sref: sref.clone(),
        })
    }
}

impl INodeInterface for Tty {
    fn open(
        &self,
        _flags: aero_syscall::OpenFlags,
        _handle: Arc<fs::file_table::FileHandle>,
    ) -> fs::Result<Option<fs::cache::DirCacheItem>> {
        let connected = self.connected.fetch_add(1, Ordering::SeqCst);
        if connected == 0 {
            super::keyboard::register_keyboard_listener(TTY.clone());

            // FIXME: This is wrong since programs assume that /dev/tty points to the controlling
            // terminal not TTY1,TTY2, etc.. This means that the Aero rendy should be another device
            // node.
            let current_task = scheduler::get_scheduler().current_task();
            current_task.attach(self.sref.upgrade().unwrap());
        }

        Ok(None)
    }

    fn close(&self, _flags: aero_syscall::OpenFlags) {
        let connected = self.connected.fetch_sub(1, Ordering::SeqCst);
        if connected == 1 {
            // We were the last process that was connected to the TTY; remove
            // the keyboard listener.
            super::keyboard::remove_keyboard_listener(TTY.clone());
        }
    }

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
        let string = core::str::from_utf8(buffer).map_err(|_| FileSystemError::NotSupported)?;

        crate::rendy::print!("{}", string);

        log::debug!("TTY::write_at(): {}", unsafe {
            core::str::from_utf8_unchecked(buffer)
        });

        Ok(buffer.len())
    }

    fn poll(&self, table: Option<&mut PollTable>) -> fs::Result<PollFlags> {
        if let Some(e) = table {
            e.insert(&self.block_queue)
        }
        let mut events = PollFlags::empty();

        if self.stdin.lock_irq().is_complete() {
            events.insert(PollFlags::IN);
        }

        Ok(events)
    }

    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            aero_syscall::TIOCGWINSZ => {
                let winsize = VirtAddr::new(arg as u64);
                let winsize = unsafe { &mut *(winsize.as_mut_ptr::<aero_syscall::WinSize>()) };

                let (rows, cols) = rendy::get_rows_cols();

                winsize.ws_row = rows as u16;
                winsize.ws_col = cols as u16;

                let (xpixel, ypixel) = rendy::get_resolution();

                winsize.ws_xpixel = xpixel as u16;
                winsize.ws_ypixel = ypixel as u16;

                Ok(0x00)
            }

            aero_syscall::TCGETS => {
                let termios = VirtAddr::new(arg as u64);
                let termios = unsafe { &mut *(termios.as_mut_ptr::<aero_syscall::Termios>()) };

                let lock = TERMIOS.lock_irq();
                let this = &*lock;

                *termios = *this;
                Ok(0x00)
            }

            aero_syscall::TCSETSF => {
                // Allow the output buffer to drain, discard pending input.
                let mut stdin = self.stdin.lock_irq();
                stdin.back_buffer.clear();
                stdin.cursor = 0;
                core::mem::drop(stdin);

                let termios = VirtAddr::new(arg as u64);
                let termios = unsafe { &*(termios.as_mut_ptr::<aero_syscall::Termios>()) };

                let mut lock = TERMIOS.lock_irq();
                let this = &mut *lock;

                *this = *termios;
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

impl TerminalDevice for Tty {
    fn attach(&self, _task: Arc<Task>) {
        // FIXME: We should handle foreground groups in TTY aswell
    }

    fn detach(&self, _task: Arc<Task>) {
        // FIXME: TTY handle
    }
}

#[cfg(target_arch = "x86_64")]
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
                rendy::print!("{}", character);
            }
        };

        let backspace = || {
            if termios.c_lflag.contains(aero_syscall::TermiosLFlag::ICANON) {
                let mut stdin = self.stdin.lock_irq();

                if stdin.back_buffer.pop().is_some()
                    && termios.c_lflag.contains(aero_syscall::TermiosLFlag::ECHO)
                {
                    rendy::backspace();
                    stdin.cursor -= 1;
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
            self.block_queue.notify_all();
            return;
        }

        match key {
            KeyCode::KEY_CAPSLOCK if !released => state.caps = !state.caps,
            KeyCode::KEY_ENTER | KeyCode::KEY_KPENTER if !released => {
                let mut stdin = self.stdin.lock_irq();

                stdin.back_buffer.push(b'\n');
                stdin.cursor = 0;

                if termios.c_lflag.contains(aero_syscall::TermiosLFlag::ECHO) {
                    rendy::print!("\n");
                }

                self.block_queue.notify_all();
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

                let (x, y) = rendy::get_cursor_position();
                rendy::set_cursor_position(x - 1, y);

                stdin.cursor -= 1;
            }

            KeyCode::KEY_RIGHT if !released => {
                let mut stdin = self.stdin.lock_irq();

                // We are at the end of the input so, we cannot shift
                // the cursor to the right anymore.
                if stdin.cursor == stdin.back_buffer.len() {
                    return;
                }

                let (x, y) = rendy::get_cursor_position();
                rendy::set_cursor_position(x + 1, y);

                stdin.advance_cursor();
            }

            _ if !released => lchar(),

            _ => {}
        }
    }
}

fn init_tty() {
    devfs::install_device(TTY.clone()).expect("failed to register tty as a device");
    ctty::init().unwrap();
}

crate::module_init!(init_tty, ModuleType::Other);
