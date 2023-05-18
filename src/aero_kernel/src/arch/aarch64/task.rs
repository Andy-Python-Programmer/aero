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

use crate::fs::cache::DirCacheItem;
use crate::mem::paging::*;
use crate::syscall::ExecArgs;
use crate::userland::vm::Vm;

pub struct ArchTask {}

impl ArchTask {
    pub fn new_idle() -> Self {
        unimplemented!()
    }

    pub fn new_kernel(entry_point: VirtAddr, enable_interrupts: bool) -> Self {
        unimplemented!()
    }

    pub fn dealloc(&self) {
        unimplemented!()
    }

    pub fn clone_process(
        &self,
        entry: usize,
        usr_stack: usize,
    ) -> Result<Self, MapToError<Size4KiB>> {
        unimplemented!()
    }

    pub fn exec(
        &mut self,
        vm: &Vm,
        executable: DirCacheItem,

        argv: Option<ExecArgs>,
        envv: Option<ExecArgs>,
    ) -> Result<(), MapToError<Size4KiB>> {
        unimplemented!()
    }

    pub fn fork(&self) -> Result<Self, MapToError<Size4KiB>> {
        unimplemented!()
    }
}

pub fn userland_last_address() -> VirtAddr {
    unimplemented!()
}

pub fn arch_task_spinup(from: &mut ArchTask, to: &ArchTask) {
    unimplemented!()
}
