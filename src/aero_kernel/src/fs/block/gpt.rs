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

use crate::fs::block::BlockDeviceInterface;

use super::BlockDevice;
use core::mem::MaybeUninit;

use alloc::boxed::Box;
use alloc::sync::Arc;

const GPT_TABLE_SIGNATURE: u64 = 0x5452415020494645;

#[repr(C)]
pub struct GptTableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub header_checksum: u32,
    pub reserved_zero: u32,
    pub current_lba: u64,
    pub backup_lba: u64,
    pub first_lba: u64,
    pub last_lba: u64,
    pub disk_guid: [u8; 16],
    /// Starting LBA of array of partition entries (usually `2` for compatibility).
    pub starting_lba: u64,
    pub num_entries: u32,
    pub entry_size: u32,
    pub table_checksum: u32,
    pub padding: [u8; 420],
}

const_assert_eq!(core::mem::size_of::<GptTableHeader>(), 512);

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct GptGuid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 2],
    e: [u8; 6],
}

impl GptGuid {
    pub const NULL: GptGuid = GptGuid {
        a: 0,
        b: 0,
        c: 0,
        d: [0; 2],
        e: [0; 6],
    };
}

const_assert_eq!(core::mem::size_of::<GptGuid>(), 16);

#[derive(Debug)]
#[repr(C)]
pub struct GptEntry {
    type_guid: GptGuid,
    unique_guid: GptGuid,
    first_lba: u64,
    last_lba: u64,
    attr_flags: u64,
    partition_name: [u8; 72],
}

impl GptEntry {
    pub fn start_lba(&self) -> u64 {
        self.first_lba
    }

    pub fn size(&self) -> u64 {
        self.last_lba - self.first_lba
    }

    pub fn partition_name(&self) -> String {
        let mut result = String::new();

        // UEFI strings are UCS-2, not UTF-16. That means that each
        // source character is exactly two bytes long.
        for i in (0..self.partition_name.len()).step_by(2) {
            let upper = u16::from(self.partition_name[i + 1]) << 8;
            let c = upper | u16::from(self.partition_name[i]);

            // Encountered a null character, so we're done.
            if c == 0 {
                break;
            }

            result.push(char::try_from(u32::from(c)).unwrap_or('�'));
        }

        result
    }

    pub fn is_used(&self) -> bool {
        self.type_guid != GptGuid::NULL
    }
}

const_assert_eq!(core::mem::size_of::<GptEntry>(), 128);

pub struct Gpt {
    entries: Box<[GptEntry]>,
}

impl Gpt {
    pub fn new(controller: Arc<BlockDevice>) -> Option<Self> {
        // Get the GPT header.
        let mut header = Box::<GptTableHeader>::new_uninit();

        controller
            .read_block(1, header.as_bytes_mut())
            .expect("gpt: failed to read first sector");

        // SAFETY: The buffer is initialized above.
        let header = unsafe { header.assume_init() };

        if header.signature != GPT_TABLE_SIGNATURE {
            return None;
        }

        let entry_size = header.entry_size as usize;
        assert_eq!(entry_size, core::mem::size_of::<GptEntry>());

        let mut entry_list = Box::<[GptEntry]>::new_uninit_slice(header.num_entries as usize);

        controller
            .read_block(
                header.starting_lba as _,
                MaybeUninit::slice_as_bytes_mut(&mut entry_list),
            )
            .expect("gpt: failed to read entry list");

        // SAFETY: The entries list is initialized above.
        let entries = unsafe { entry_list.assume_init() };

        return Some(Self { entries });
    }

    pub fn entries(&self) -> &[GptEntry] {
        &self.entries
    }
}
