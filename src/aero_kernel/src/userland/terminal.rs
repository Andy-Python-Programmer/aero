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

use aero_syscall::signal;

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
}

impl LineDiscipline {
    /// Creates a new line discipline.
    pub fn new() -> Self {
        Self {
            wq: WaitQueue::new(),
            buffer: Mutex::new(Vec::new()),
            foreground: RwLock::new(Weak::default()),
        }
    }

    /// Reads data from the line discipline buffer.
    pub fn read(&self, target: &mut [u8]) -> Result<usize, SignalError> {
        let mut buffer = self.wq.block_on(&self.buffer, |e| !e.is_empty())?;

        let size = core::cmp::min(target.len(), buffer.len());
        target[..size].copy_from_slice(&buffer.drain(..size).collect::<Vec<_>>());

        Ok(size)
    }

    /// Writes data to the line discipline buffer.
    pub fn write(&self, target: &[u8]) {
        let mut buffer = self.buffer.lock_irq();

        for byte in target {
            match byte {
                // ETX: End of Text (`Ctrl+C`)
                0x3 => {
                    if let Some(foreground) = self.foreground() {
                        foreground.signal(signal::SIGINT);
                    }
                }

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
