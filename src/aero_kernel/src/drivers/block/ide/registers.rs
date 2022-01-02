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

use bit_field::BitField;

use crate::drivers::block::ahci::AtaCommand;
use crate::mem::paging::PhysAddr;

use crate::utils::io;
use crate::utils::io::BasedPort;

const BASE_FEATURE: u16 = 1;
const BASE_SECTOR_COUNT: u16 = 2;
const BASE_LBA_LO: u16 = 3;
const BASE_LBA_MID: u16 = 4;
const BASE_LBA_HI: u16 = 5;
const BASE_DRIVE_SEL: u16 = 6;
const BASE_STATUS: u16 = 7;
const BASE_COMMAND: u16 = 7;

const CTRL_DEV_CTRL: u16 = 0;

const BMIDE_COMMAND: u16 = 0;
const BMIDE_STATUS: u16 = 2;
const BMIDE_PRDT: u16 = 4;

bitflags::bitflags! {
    pub struct BaseErrorReg: u8 {
        const AMNF = 0b00000001; // Address mark not found
        const TKZNF = 0b00000010; // Track zero not found
        const ABRT = 0b00000100; // Aborted command
        const MCR = 0b00001000; // Media change requested
        const IDNF = 0b00010000; // ID not found
        const MC = 0b00100000; // Media changed
        const UNC = 0b01000000; // Uncorrectable data error
        const BBK = 0b10000000; // Bad Block detected
    }
}

pub struct BaseDriveSelReg(u8);

impl BaseDriveSelReg {
    pub fn new() -> BaseDriveSelReg {
        BaseDriveSelReg(0b10100000)
    }

    pub fn val(&self) -> u8 {
        self.0
    }

    pub fn set_block_num(&mut self, num: usize) {
        self.0.set_bits(0..=3, num as u8);
    }

    pub fn set_slave(&mut self, slave: bool) {
        self.0.set_bit(4, slave);
    }

    pub fn set_lba(&mut self, lba: bool) {
        self.0.set_bit(6, lba);
    }
}

bitflags::bitflags! {
    pub struct BaseStatusReg: u8 {
        const ERR = 0b00000001; // Error occured
        const IDX = 0b00000010; // Index. Always set to zero
        const CORR = 0b00000100; // Corrected data. Always set to zero
        const DRQ = 0b00001000; // Set when the drive has PIO data to transfer, or is ready to accept PIO data
        const SRV = 0b00010000; // Overlapped Mode Service Request
        const DF = 0b00100000; // Drive Fault Error (does not set ERR)
        const RDY = 0b01000000; // Bit is clear when drive is spun down, or after an error. Set otherwise
        const BSY = 0b10000000; // Indicates the drive is preparing to send / receive data
    }
}

bitflags::bitflags! {
    pub struct CtrlDevCtrlReg: u8 {
        const NIEN = 0b00000010; // Disable interrupts
        const SRST = 0b00000100; // Software reset
        const HOB = 0b10000000; // Set to read back High Order Byte of the last LBA48 sent
    }
}

bitflags::bitflags! {
    pub struct CtrlDriveAddrReg: u8 {
        const DS0 = 0b00000001; // Drive 0 select. Clears when drive 0 selected
        const DS1 = 0b00000010; // Drive 1 select. Clears when drive 1 selected
        const WTG = 0b01000000; // Write gate; goes low while writing to the drive is in progress
    }
}

bitflags::bitflags! {
    pub struct BMIdeCmd: u8 {
        const DMA_START = 0b00000001;
        const DMA_READ = 0b00001000;
    }
}

bitflags::bitflags! {
    pub struct BMIdeStatus: u8 {
        const DMA_ACTIVE = 0b0000_0001;
        const DMA_FAILED = 0b0000_0010;
        const DISK_IRQ = 0b0000_0100;
        const MASTER_DMA_CAPPABLE = 0b0010_0000;
        const SLAVE_DMA_CAPPABLE = 0b0100_0000;
        const NO_DMA_SHARING = 0b1000_0000;
    }
}

pub struct DevBaseReg {
    base: BasedPort,
}

impl DevBaseReg {
    pub fn new(base: u16) -> DevBaseReg {
        DevBaseReg {
            base: BasedPort::new(base),
        }
    }

    pub fn clear_features(&mut self) {
        self.base.write_offset(BASE_FEATURE, 0u8);
    }

    pub fn status(&self) -> BaseStatusReg {
        BaseStatusReg::from_bits_truncate(self.base.read_offset::<u8>(BASE_STATUS))
    }

    pub fn try_status(&self) -> Option<BaseStatusReg> {
        BaseStatusReg::from_bits(self.base.read_offset::<u8>(BASE_STATUS))
    }

    pub fn set_drive_select(&mut self, slave: bool, lba: bool, lba28_block_num: u16) {
        let mut sel = BaseDriveSelReg::new();
        sel.set_block_num(lba28_block_num as usize);
        sel.set_lba(lba);
        sel.set_slave(slave);
        self.base.write_offset(BASE_DRIVE_SEL, sel.val());
    }

    pub fn set_command(&mut self, cmd: AtaCommand) {
        self.base.write_offset(BASE_COMMAND, cmd as u8);
    }

    pub fn set_sector_count_lba28(&mut self, count: u8) {
        self.base.write_offset(BASE_SECTOR_COUNT, count);
    }

    pub fn set_sector_count_lba48(&mut self, count: u16) {
        self.base
            .write_offset(BASE_SECTOR_COUNT, count.get_bits(8..16) as u8);
        self.base
            .write_offset(BASE_SECTOR_COUNT, count.get_bits(0..8) as u8);
    }

    pub fn set_sector_count(&mut self, lba48: bool, count: u16) {
        match lba48 {
            true => self.set_sector_count_lba48(count),
            false => {
                assert!(count < 256);
                self.set_sector_count_lba28(count as u8)
            }
        }
    }

    pub fn set_sector_num_lba48(&mut self, sector: usize) {
        self.base
            .write_offset(BASE_LBA_LO, sector.get_bits(24..32) as u8);
        self.base
            .write_offset(BASE_LBA_MID, sector.get_bits(32..40) as u8);
        self.base
            .write_offset(BASE_LBA_HI, sector.get_bits(40..48) as u8);
        self.base
            .write_offset(BASE_LBA_LO, sector.get_bits(0..8) as u8);
        self.base
            .write_offset(BASE_LBA_MID, sector.get_bits(8..16) as u8);
        self.base
            .write_offset(BASE_LBA_HI, sector.get_bits(16..24) as u8);
    }

    pub fn set_sector_num_lba28(&mut self, sector: usize) {
        self.base
            .write_offset(BASE_LBA_LO, sector.get_bits(0..8) as u8);
        self.base
            .write_offset(BASE_LBA_MID, sector.get_bits(8..16) as u8);
        self.base
            .write_offset(BASE_LBA_HI, sector.get_bits(16..24) as u8);
    }

    pub fn lba_mid(&self) -> u8 {
        self.base.read_offset::<u8>(BASE_LBA_MID)
    }

    pub fn lba_hi(&self) -> u8 {
        self.base.read_offset::<u8>(BASE_LBA_HI)
    }

    pub fn set_sector_num(&mut self, lba48: bool, sector: usize) {
        match lba48 {
            true => self.set_sector_num_lba48(sector),
            false => self.set_sector_num_lba28(sector),
        }
    }
}

pub struct DevCtrlReg {
    base: BasedPort,
}

impl DevCtrlReg {
    pub fn new(base: u16) -> DevCtrlReg {
        DevCtrlReg {
            base: BasedPort::new(base),
        }
    }

    pub fn software_reset(&mut self) {
        let dev = CtrlDevCtrlReg::SRST;
        self.base.write_offset(CTRL_DEV_CTRL, dev.bits());
        io::delay(5000);
        self.base.write_offset(CTRL_DEV_CTRL, 0u8);
    }

    pub fn enable_interrupts(&mut self) {
        self.base.write_offset(CTRL_DEV_CTRL, 0u8);
    }
}

pub struct BusMasterReg {
    base: BasedPort,
}

impl BusMasterReg {
    pub fn new(base: u16) -> BusMasterReg {
        BusMasterReg {
            base: BasedPort::new(base),
        }
    }

    pub fn start_dma(&mut self, cmd: AtaCommand) {
        let mut c = BMIdeCmd::DMA_START;

        if !cmd.is_write() {
            c.insert(BMIdeCmd::DMA_READ);
        }

        self.base.write_offset(BMIDE_COMMAND, c.bits());
    }

    pub fn load_prdt(&mut self, prdt_base: PhysAddr) {
        self.base
            .write_offset(BMIDE_PRDT, prdt_base.as_u64() as u32);
    }

    pub fn status(&self) -> BMIdeStatus {
        BMIdeStatus::from_bits_truncate(self.base.read_offset::<u8>(BMIDE_STATUS))
    }

    pub fn ack_interrupt(&mut self) {
        let v = BMIdeStatus::DISK_IRQ | BMIdeStatus::DMA_FAILED;
        self.base.write_offset(BMIDE_STATUS, v.bits());
    }
}
