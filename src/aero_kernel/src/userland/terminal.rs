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

use aero_syscall::{signal, Termios, TermiosIFlag, TermiosLFlag};

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use spin::RwLock;

use crate::fs::inode::INodeInterface;
use crate::utils::sync::{Mutex, WaitQueue};

use super::signals::SignalError;
use super::task::sessions::{Group, SESSIONS};
use super::task::Task;

/// Terminal Device
#[downcastable]
pub trait TerminalDevice: Send + Sync + INodeInterface {
    /// Called when a session leader is attached/connected to the terminal device.
    ///
    /// The processs group of `task` will be set as the active process group that is associated with
    /// the terminal (foreground).
    fn attach(&self, task: Arc<Task>);
    fn detach(&self, task: Arc<Task>);
}

#[derive(Debug, Copy, Clone)]
pub enum LineControl {
    Echo(u8),
}

/// Line Discipline
///
/// The middle terminal subsystem layer, used to implement behavior common to terminal devices.
///
/// For example, the line discipline handles special characters such as the interrupt character
/// (`Ctrl+C`), the erase character (Backspace or Delete), and the kill character (`Ctrl+U`) on
/// input.
pub struct LineDiscipline {
    wq: WaitQueue,
    buffer: Mutex<Vec<u8>>,
    foreground: RwLock<Weak<Group>>,
    // TODO: Make this private.
    pub termios: Mutex<Termios>,
}

impl LineDiscipline {
    /// Creates a new line discipline.
    pub fn new() -> Self {
        use aero_syscall::{TermiosCFlag, TermiosOFlag};

        // converts `^X` into `X`
        let ctrl = |c| (c as u8 - 0x40);

        let mut termios = Termios {
            c_iflag: TermiosIFlag::ICRNL | TermiosIFlag::IXON,
            c_oflag: TermiosOFlag::OPOST | TermiosOFlag::ONLCR,
            c_cflag: TermiosCFlag::CS6 | TermiosCFlag::CS7 | TermiosCFlag::CREAD,
            c_lflag: TermiosLFlag::ECHOKE
                | TermiosLFlag::ECHOE
                | TermiosLFlag::ECHOK
                | TermiosLFlag::ECHO
                | TermiosLFlag::ECHOCTL
                | TermiosLFlag::ISIG
                | TermiosLFlag::ICANON
                | TermiosLFlag::IEXTEN,
            c_line: 0,
            c_cc: [0; 32],
            c_ispeed: 0,
            c_ospeed: 0,
        };

        termios.c_cc[aero_syscall::VINTR] = ctrl('C');
        termios.c_cc[aero_syscall::VQUIT] = ctrl('\\');
        termios.c_cc[aero_syscall::VERASE] = 127; // DEL character
        termios.c_cc[aero_syscall::VKILL] = ctrl('U');
        termios.c_cc[aero_syscall::VEOF] = ctrl('D');
        termios.c_cc[aero_syscall::VMIN] = 1;
        termios.c_cc[aero_syscall::VSTART] = ctrl('Q');
        termios.c_cc[aero_syscall::VSTOP] = ctrl('S');
        termios.c_cc[aero_syscall::VSUSP] = ctrl('Z');

        Self {
            wq: WaitQueue::new(),
            buffer: Mutex::new(Vec::new()),
            foreground: RwLock::new(Weak::default()),
            termios: Mutex::new(termios),
        }
    }

    #[inline]
    pub fn termios(&self) -> Termios {
        self.termios.lock().clone()
    }

    #[inline]
    pub fn set_termios(&self, termios: Termios) {
        *self.termios.lock() = termios;
    }

    pub fn read(&self, target: &mut [u8]) -> Result<usize, SignalError> {
        let mut buffer = self.wq.block_on(&self.buffer, |buf| !buf.is_empty())?;

        let size = core::cmp::min(target.len(), buffer.len());
        target[..size].copy_from_slice(&buffer.drain(..size).collect::<Vec<_>>());

        Ok(size)
    }

    pub fn write<F>(&self, target: &[u8], callback: F)
    where
        F: Fn(LineControl),
    {
        let mut buffer = self.buffer.lock_irq();
        let termios = self.termios.lock();
        let should_echo = termios.c_lflag.contains(TermiosLFlag::ECHO);

        for byte in target {
            match byte {
                // ETX: End of Text (`Ctrl+C`)
                0x3 if termios.is_cooked() => {
                    if let Some(foreground) = self.foreground() {
                        foreground.signal(signal::SIGINT);
                    }
                }

                b'\r' if termios.c_iflag.contains(TermiosIFlag::ICRNL) => {
                    buffer.push(b'\n');

                    if should_echo {
                        callback(LineControl::Echo(b'\r'));
                        callback(LineControl::Echo(b'\n'));
                    }
                }

                byte if termios.is_cooked() => {
                    buffer.push(*byte);

                    if should_echo {
                        callback(LineControl::Echo(*byte))
                    }
                }

                // In raw mode:
                _ => buffer.push(*byte),
            }
        }

        drop(buffer);
        self.wq.notify_all();
    }

    pub fn foreground(&self) -> Option<Arc<Group>> {
        self.foreground.read().upgrade()
    }

    pub fn set_foreground(&self, task: Arc<Task>) {
        *self.foreground.write() = Arc::downgrade(&SESSIONS.find_group(task).unwrap());
    }

    /// Returns whether the line discipline buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.lock_irq().is_empty()
    }

    /// Returns the line discipline's wait queue.
    pub fn wait_queue(&self) -> &WaitQueue {
        &self.wq
    }
}
