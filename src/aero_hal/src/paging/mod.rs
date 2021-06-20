/*
 * Copyright (C) 2021 The Aero Project Developers.
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

use self::{address::VirtualAddress, mapper::Mapper, page::PageTable};

pub mod address;
pub mod frame;
pub mod mapper;
pub mod page;

pub trait PageSize: Copy + Clone + Eq + PartialOrd + Ord {
    /// The page size in bytes.
    const SIZE: u64;
}

pub trait NotGiantPageSize: PageSize {}

macro_rules! impl_size_t {
    ($enum:ident, $size:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        pub enum $enum {}

        impl PageSize for $enum {
            const SIZE: u64 = $size;
        }
    };
}

impl_size_t!(Size4KiB, 4096);
impl_size_t!(Size2MiB, Size4KiB::SIZE * 512);
impl_size_t!(Size1GiB, Size2MiB::SIZE * 512);

impl NotGiantPageSize for Size4KiB {}
impl NotGiantPageSize for Size2MiB {}

#[repr(transparent)]
pub struct ActivePageTable<'mapper>(Mapper<'mapper>);

impl<'mapper> ActivePageTable<'mapper> {
    pub fn new(
        level_4_table: &'mapper mut PageTable,
        physical_memory_offset: VirtualAddress,
    ) -> Self {
        Self(Mapper::new(level_4_table, physical_memory_offset))
    }
}
