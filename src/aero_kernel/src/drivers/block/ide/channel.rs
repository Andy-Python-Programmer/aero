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

use alloc::sync::Arc;
use bit_field::BitField;

use super::registers::*;

use crate::drivers::block::ahci::{AtaCommand, DmaBuffer, DmaRequest};
use crate::mem::paging::*;

use crate::utils::io::delay;
use crate::utils::sync::Mutex;

struct PrdTable<'a> {
    data: &'a mut [PrdEntry],
}

impl<'a> PrdTable<'a> {
    pub fn new(addr: PhysAddr, entries: usize) -> PrdTable<'a> {
        let mapped_addr = unsafe { crate::PHYSICAL_MEMORY_OFFSET + addr.as_u64() };
        let ptr = mapped_addr.as_mut_ptr::<PrdEntry>();
        let entries = unsafe { core::slice::from_raw_parts_mut(ptr, entries) };

        PrdTable::<'a> { data: entries }
    }

    pub fn entry_at(&mut self, idx: usize) -> &mut PrdEntry {
        &mut self.data[idx]
    }

    pub fn load_dma(&mut self, buf: &[DmaBuffer], sectors: usize) -> usize {
        let mut rem = sectors;

        for (i, b) in buf.iter().enumerate() {
            let cur_sectors = b.sectors();

            let is_last = (i == buf.len() - 1) || (rem == cur_sectors);

            let entry = self.entry_at(i);

            entry.set_addr(b.start());
            entry.set_byte_count(b.data_size());
            entry.set_last_entry(is_last);

            rem -= cur_sectors;

            if is_last {
                break;
            }
        }

        sectors - rem
    }
}

#[repr(C)]
struct PrdEntry {
    addr: u32,
    cnt: u32,
}

impl PrdEntry {
    pub fn set_addr(&mut self, addr: PhysAddr) {
        self.addr = addr.as_u64() as u32;
    }

    pub fn set_byte_count(&mut self, bytes: usize) {
        self.cnt.set_bits(0..16, bytes as u32);
    }

    pub fn set_last_entry(&mut self, last: bool) {
        self.cnt.set_bit(31, last);
    }
}

struct IdeChannelData {
    base: DevBaseReg,
    ctrl: DevCtrlReg,
    bmide: BusMasterReg,
    interrupt_nr: usize,
    prdt_addr: PhysAddr,
    active_cmd: Option<Arc<DmaRequest>>,
}

impl IdeChannelData {
    pub fn new(base: u16, ctrl: u16, bmide: u16, interrupt_nr: usize) -> IdeChannelData {
        IdeChannelData {
            base: DevBaseReg::new(base),
            ctrl: DevCtrlReg::new(ctrl),
            bmide: BusMasterReg::new(bmide),
            interrupt_nr,
            prdt_addr: PhysAddr::new(0),
            active_cmd: None,
        }
    }

    pub fn software_reset(&mut self) {
        self.ctrl.software_reset();
    }

    pub fn detect(&mut self, slave: bool) -> bool {
        self.software_reset();

        let mut sel = BaseDriveSelReg::new();
        sel.set_slave(slave);

        self.base.set_drive_select(slave, false, 0);
        delay(1000);

        self.base.set_command(AtaCommand::AtaCommandIdentifyDevice);
        delay(1000);

        let status = self.base.status();

        if status == BaseStatusReg::empty() {
            return false;
        }

        loop {
            if let Some(status) = self.base.try_status() {
                if status.contains(BaseStatusReg::ERR) {
                    return false;
                }
                if !status.contains(BaseStatusReg::BSY) && status.contains(BaseStatusReg::DRQ) {
                    break;
                }
            } else {
                return false;
            }
        }

        let lm = self.base.lba_mid();
        let lh = self.base.lba_hi();

        match (lm, lh) {
            (0x0, 0x0) => {
                return true;
            }

            _ => {}
        }

        return false;
    }

    pub fn setup_prdt(&mut self) {
        let prdt = pmm_alloc(BuddyOrdering::Size4KiB);

        self.bmide.load_prdt(prdt);
        self.prdt_addr = prdt;
    }

    pub fn enable_interrupts(&mut self) {
        // FIXME
        self.ctrl.enable_interrupts();
    }

    pub fn init(&mut self) {
        self.enable_interrupts();
        self.setup_prdt();

        let _status = self.bmide.status();
    }

    pub fn get_prdt(&mut self) -> PrdTable {
        PrdTable::new(self.prdt_addr, 8192 / 8)
    }

    pub fn run_ata_command(
        &mut self,
        cmd: AtaCommand,
        sector: usize,
        count: usize,
        buf: &[DmaBuffer],
        slave: bool,
    ) -> usize {
        self.base.clear_features();

        let mut table = self.get_prdt();

        let count = table.load_dma(buf, count);

        let is_lba48 = cmd.is_lba48();

        self.base.set_drive_select(
            slave,
            true,
            if is_lba48 {
                0
            } else {
                sector.get_bits(24..28) as u16
            },
        );

        self.base.set_sector_count(is_lba48, count as u16);
        self.base.set_sector_num(is_lba48, sector);
        self.base.set_command(cmd);

        self.bmide.ack_interrupt();

        self.bmide.start_dma(cmd);

        count
    }

    pub fn run_request(&mut self, request: Arc<DmaRequest>, offset: usize, slave: bool) -> usize {
        self.active_cmd = Some(request.clone());

        let rem = request.count - offset;
        let max_count = 256;

        let cnt = core::cmp::min(rem, max_count);

        let cnt = self.run_ata_command(
            request.into_command(),
            request.sector() + offset,
            cnt,
            request.at_offset(offset),
            slave,
        );

        offset + cnt
    }
}

pub struct IdeChannel {
    data: Mutex<IdeChannelData>,
}

impl IdeChannel {
    pub fn new(base: u16, ctrl: u16, bmide: u16, interrupt_nr: usize) -> Arc<IdeChannel> {
        Arc::new(IdeChannel {
            data: Mutex::new(IdeChannelData::new(base, ctrl, bmide, interrupt_nr)),
        })
    }

    pub fn detect(&self, slave: bool) -> bool {
        self.data.lock_irq().detect(slave)
    }

    pub fn init(&self) {
        self.data.lock_irq().init();
    }

    pub fn run_request(&self, request: Arc<DmaRequest>, slave: bool) -> Option<usize> {
        let mut offset = 0;

        while offset < request.count {
            offset = self
                .data
                .lock_irq()
                .run_request(request.clone(), offset, slave);
        }

        Some(request.count * 512)
    }
}
