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

use aero_syscall::*;

use crate::message::IncomingMessage;

pub struct EventLoop {
    buffer: Vec<u8>,
}

impl EventLoop {
    pub fn new() -> Self {
        let mut buffer = Vec::new();

        buffer.resize(1024, 0);

        Self { buffer }
    }

    pub fn receive_with_flags(
        &mut self,
        flags: IpcRecvFlags,
    ) -> Result<IncomingMessage, AeroSyscallError> {
        let mut pid = 0;
        let mut length = 0;
        let mut tag = 0;

        match sys_ipc_recv(&mut self.buffer, &mut pid, &mut length, &mut tag, flags) {
            Ok(id) => Ok(IncomingMessage::new(id, pid, tag, &self.buffer[0..length])),
            Err(AeroSyscallError::E2BIG) => {
                self.buffer.resize(length, 0);
                self.receive_with_flags(flags)
            }
            Err(err) => Err(err),
        }
    }

    pub fn receive(&mut self) -> Result<IncomingMessage, AeroSyscallError> {
        self.receive_with_flags(IpcRecvFlags::empty())
    }
}
