/*
 * Copyright (C) 2021-2023 The Aero Project Developers.
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

use crate::drivers::pci::*;
use crate::mem::paging::*;

const TX_DESC_NUM: usize = 32;
const TX_DESC_SIZE: usize = TX_DESC_NUM * core::mem::size_of::<TxDescriptor>();

const RX_QUEUE_SIZE: usize = 32;

#[derive(Copy, Clone, Debug)]
enum Error {
    UnknownBar,
    NoEeprom,
    OutOfMemory,
    NotSupported,
}

#[derive(Copy, Clone)]
#[repr(usize)]
enum Register {
    Control = 0x00,
    Eeprom = 0x14,

    TCtrl = 0x400,
    /// Lower bits of the 64 bit descriptor base address.
    TxDesLo = 0x3800,
    /// Upper 32 bits of the 64 bit descriptor base address.
    TxDesHi = 0x3804,
    /// Descriptor length and must be 128B aligned.
    TxDescLen = 0x3808,
    /// Head pointer for the transmit descriptor ring.
    TxDescHead = 0x3810,
    /// Tail pointer for the transmit descriptor ring.
    TxDescTail = 0x3818,
    /// Controls the IPG (Inter Packet Gap) timer.
    Tipg = 0x410,
}

bitflags::bitflags! {
    struct ControlFlags: u32 {
        const LRST    = 1 << 3;
        const ASDE    = 1 << 5;
        const SLU     = 1 << 6;
        const ILOS    = 1 << 7;
        const RST     = 1 << 26;
        const VME     = 1 << 30;
        const PHY_RST = 1 << 31;
    }
}

bitflags::bitflags! {
    pub struct TStatus: u8 {
        const DD = 1 << 0; // Descriptor Done
        const EC = 1 << 1; // Excess Collisions
        const LC = 1 << 2; // Late Collision
        const TU = 1 << 3; // Transmit Underrun
    }
}

bitflags::bitflags! {
    pub struct TCtl: u32 {
        const EN     = 1 << 1;  // Transmit Enable
        const PSP    = 1 << 3;  // Pad Short Packets
        const SWXOFF = 1 << 22; // Software XOFF Transmission
        const RTLC   = 1 << 24; // Re-transmit on Late Collision
    }
}

impl TCtl {
    /// Sets the number of attempts at retransmission prior to giving
    /// up on the packet (not including the first transmission attempt).
    pub fn set_collision_threshold(&mut self, value: u8) {
        self.bits |= (value as u32) << 4;
    }

    /// Sets the minimum number of byte times which must elapse for
    /// proper CSMA/CD operation.
    pub fn set_collision_distance(&mut self, value: u8) {
        self.bits |= (value as u32) << 12;
    }
}

#[repr(C, packed)]
pub struct TxDescriptor {
    pub addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: TStatus,
    pub css: u8,
    pub special: u16,
}

struct Eeprom<'a> {
    e1000: &'a E1000,
}

impl<'a> Eeprom<'a> {
    fn new(e1000: &'a E1000) -> Self {
        Self { e1000 }
    }

    fn read(&self, addr: u8) -> u32 {
        self.e1000.write(Register::Eeprom, 1 | ((addr as u32) << 8));

        loop {
            let res = self.e1000.read(Register::Eeprom);

            if res & (1 << 4) > 0 {
                return (res >> 16) & 0xffff;
            }
        }
    }
}

struct E1000 {
    base: VirtAddr,
}

impl E1000 {
    fn new(header: &PciHeader) -> Result<(), Error> {
        header.enable_bus_mastering();
        header.enable_mmio();

        let bar0 = header.get_bar(0).ok_or(Error::UnknownBar)?;

        let registers_addr = match bar0 {
            Bar::Memory64 { address, .. } => PhysAddr::new(address),
            Bar::Memory32 { address, .. } => PhysAddr::new(address as u64),
            _ => return Err(Error::UnknownBar),
        };

        let this = Self {
            base: registers_addr.as_hhdm_virt(),
        };

        this.reset();

        if !this.detect_eeprom() {
            return Err(Error::NoEeprom);
        }

        let eeprom = Eeprom::new(&this);

        let mut mac = [0u8; 6];

        // Get the MAC address
        for i in 0..3 {
            let x = eeprom.read(i) as u16;
            mac[i as usize * 2] = (x & 0xff) as u8;
            mac[i as usize * 2 + 1] = (x >> 8) as u8;
        }

        log::trace!(
            "e1000: MAC address {:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5]
        );

        this.init_tx()?;
        this.init_rx()?;

        log::trace!("e1000: successfully initialized");
        Ok(())
    }

    fn init_tx(&self) -> Result<(), Error> {
        assert!(core::mem::size_of::<TxDescriptor>() * TX_DESC_NUM < Size4KiB::SIZE as usize);

        let frame: PhysFrame<Size4KiB> =
            FRAME_ALLOCATOR.allocate_frame().ok_or(Error::OutOfMemory)?;

        let addr = frame.start_address().as_hhdm_virt();
        let descriptors = addr
            .read_mut::<[TxDescriptor; TX_DESC_NUM]>()
            .ok_or(Error::NotSupported)?;

        for desc in descriptors {
            desc.status = TStatus::DD;
        }

        let phys = frame.start_address();

        self.write(Register::TxDesLo, phys.as_u64() as _);
        self.write(Register::TxDesHi, (phys.as_u64() >> 32) as _);
        self.write(Register::TxDescLen, TX_DESC_SIZE as _);
        self.write(Register::TxDescHead, 0);
        self.write(Register::TxDescTail, 0);

        let mut flags = TCtl::EN | TCtl::PSP | TCtl::RTLC;
        flags.set_collision_distance(64);
        flags.set_collision_threshold(15);

        self.write(Register::TCtrl, flags.bits());

        // TODO: Set the default values for the Tx Inter Packet
        //       Gap Timer.
        // self.write(Register::Tipg, 0x??????)

        Ok(())
    }

    fn init_rx(&self) -> Result<(), Error> {
        Ok(())
    }

    fn detect_eeprom(&self) -> bool {
        self.write(Register::Eeprom, 1);

        for _ in 0..1000 {
            let value = self.read(Register::Eeprom);

            if value & (1 << 4) > 0 {
                return true;
            }
        }

        false
    }

    fn reset(&self) {
        self.insert_flags(Register::Control, ControlFlags::RST.bits());

        while ControlFlags::from_bits_truncate(self.read(Register::Control))
            .contains(ControlFlags::RST)
        {
            core::hint::spin_loop();
        }

        // Do not use VLANs, clear reset and do not invert loss-of-signal.
        self.remove_flags(
            Register::Control,
            (ControlFlags::LRST | ControlFlags::PHY_RST | ControlFlags::VME).bits(),
        );
    }

    fn remove_flags(&self, register: Register, flag: u32) {
        self.write(register, self.read(register) & !flag);
    }

    fn insert_flags(&self, register: Register, flag: u32) {
        self.write(register, self.read(register) | flag);
    }

    fn write(&self, register: Register, value: u32) {
        unsafe {
            let register = self.base.as_mut_ptr::<u8>().add(register as usize);
            core::ptr::write_volatile(register as *mut u32, value);
        }
    }

    fn read(&self, register: Register) -> u32 {
        unsafe { self.read_raw(register as u32) }
    }

    unsafe fn read_raw(&self, register: u32) -> u32 {
        let register = self.base.as_ptr::<u8>().add(register as usize);
        core::ptr::read_volatile(register as *const u32)
    }
}

struct Handler;

impl Handler {
    fn new() -> Arc<Self> {
        Arc::new(Self {})
    }
}

impl PciDeviceHandle for Handler {
    fn handles(&self, vendor_id: Vendor, device_id: DeviceType) -> bool {
        vendor_id == Vendor::Intel && device_id == DeviceType::EthernetController
    }

    fn start(&self, header: &PciHeader, _offset_table: &mut OffsetPageTable) {
        E1000::new(header).unwrap()
    }
}

fn init() {
    register_device_driver(Handler::new())
}

crate::module_init!(init, ModuleType::Block);
