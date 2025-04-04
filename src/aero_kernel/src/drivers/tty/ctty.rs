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

//! `/dev/tty`: Controlling terminal of the current process.

use aero_syscall::{OpenFlags, TIOCNOTTY};
use alloc::sync::{Arc, Weak};

use crate::fs::devfs::Device;
use crate::fs::inode::{FileType, INodeInterface, Metadata, PollFlags, PollTable};
use crate::fs::{self, devfs, FileSystemError};
use crate::userland::scheduler;

struct Ctty {
    device_id: usize,
    sref: Weak<Self>,
}

impl Ctty {
    fn new() -> Arc<Self> {
        Arc::new_cyclic(|sref| Self {
            device_id: devfs::alloc_device_marker(),
            sref: sref.clone(),
        })
    }

    /// Returns the controlling terminal device of the current process.
    fn controlling_terminal() -> fs::Result<Arc<dyn INodeInterface>> {
        let current_task = scheduler::get_scheduler().current_task();
        let terminal_device = current_task.controlling_terminal();

        terminal_device
            .map(|device| device as Arc<dyn INodeInterface>)
            .ok_or(FileSystemError::NoTty)
    }

    #[inline]
    fn sref(&self) -> Arc<dyn INodeInterface> {
        self.sref.upgrade().unwrap()
    }
}

impl INodeInterface for Ctty {
    fn metadata(&self) -> fs::Result<Metadata> {
        Ok(Metadata {
            id: 0,
            file_type: FileType::Device,
            size: 0,
            children_len: 0,
        })
    }

    fn read_at(&self, flags: OpenFlags, offset: usize, buffer: &mut [u8]) -> fs::Result<usize> {
        Self::controlling_terminal()?.read_at(flags, offset, buffer)
    }

    fn write_at(&self, offset: usize, buffer: &[u8]) -> fs::Result<usize> {
        Self::controlling_terminal()?.write_at(offset, buffer)
    }

    fn poll(&self, table: Option<&mut PollTable>) -> fs::Result<PollFlags> {
        Self::controlling_terminal()?.poll(table)
    }

    /// ### Supported `ioctl(2)` requests
    ///
    /// In addition to the `ioctl(2)` requests supported by the device that CTTY refers to, the
    /// `ioctl(2)` request `TIOCNOTTY` is supported.
    fn ioctl(&self, command: usize, arg: usize) -> fs::Result<usize> {
        match command {
            TIOCNOTTY => {
                let current_task = scheduler::get_scheduler().current_task();
                current_task.detach();

                Ok(0)
            }

            _ => Self::controlling_terminal()?.ioctl(command, arg),
        }
    }
}

impl Device for Ctty {
    #[inline]
    fn device_marker(&self) -> usize {
        self.device_id
    }

    #[inline]
    fn device_name(&self) -> String {
        String::from("tty")
    }

    #[inline]
    fn inode(&self) -> Arc<dyn INodeInterface> {
        self.sref()
    }
}

/// Registers the `/dev/ctty` character device.
pub fn init() -> fs::Result<()> {
    devfs::install_device(Ctty::new())?;
    Ok(())
}
