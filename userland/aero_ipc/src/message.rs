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

#[derive(Debug)]
pub struct IncomingMessage<'a> {
    id: usize,
    pid: usize,
    tag: usize,
    data: &'a [u8],
}

impl<'a> IncomingMessage<'a> {
    pub(crate) fn new(id: usize, pid: usize, tag: usize, data: &'a [u8]) -> Self {
        Self { id, pid, tag, data }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn pid(&self) -> usize {
        self.pid
    }

    pub fn tag(&self) -> usize {
        self.tag
    }

    pub fn data(&self) -> &[u8] {
        self.data
    }
}
