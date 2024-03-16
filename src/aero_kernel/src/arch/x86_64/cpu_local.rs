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

use core::alloc::Layout;
use core::ops::{Deref, DerefMut};

use crate::extern_sym;
use crate::mem::paging::VirtAddr;

use super::io;

#[repr(C)]
pub struct CpuLocal<T>(T);

impl<T> CpuLocal<T> {
    pub const fn new(val: T) -> Self {
        Self(val)
    }

    pub fn addr(&self) -> VirtAddr {
        let val: u64;

        unsafe {
            // gs:[0] -> SELF_PTR
            asm!(
                "mov {}, qword ptr gs:[0]",
                lateout(reg) val,
                options(nostack, preserves_flags, pure, readonly),
            );
        }

        let self_addr = VirtAddr::new(self as *const _ as u64);
        let section_addr = VirtAddr::new(extern_sym!(__cpu_local_start) as u64);

        let offset = self_addr - section_addr;
        VirtAddr::new(val) + offset
    }
}

impl<T> Deref for CpuLocal<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.addr().as_ptr() }
    }
}

impl<T> DerefMut for CpuLocal<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.addr().as_mut_ptr() }
    }
}

/// The GS register holds a pointer to CPU-local data at a fixed offset of 0. While this approach
/// requires an additional memory lookup for accessing the data, it enables better code optimization
/// since the subsequent access is just a normal memory access. Considering the size of the
/// CPU-local data, this optimization is beneficial.
#[cpu_local(subsection = "self_ptr")]
static SELF_PTR: u64 = 0;

#[cpu_local]
static mut CPUID: usize = 0;

pub fn init(cpu_id: usize) {
    let start = VirtAddr::new(extern_sym!(__cpu_local_start).addr() as u64);
    let end = VirtAddr::new(extern_sym!(__cpu_local_end).addr() as u64);

    unsafe {
        let size = end - start;

        let layout = Layout::from_size_align_unchecked(size as _, 64);
        let data = alloc::alloc::alloc_zeroed(layout);

        core::ptr::copy_nonoverlapping::<u8>(start.as_ptr(), data, size as usize);
        *data.cast::<u64>() = data as u64;

        io::wrmsr(io::IA32_GS_BASE, data as u64);
        *CPUID = cpu_id;
    }
}
