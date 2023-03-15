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
use spin::Once;

use crate::acpi::aml;
use crate::arch::interrupts::{self, InterruptStack};
use crate::drivers::pci::*;
use crate::mem::paging::*;
use crate::userland::scheduler;
use crate::utils::sync::{BlockQueue, Mutex};

use crate::net::{self, ethernet, NetworkDevice};
use crate::net::{MacAddr, NetworkDriver, PacketBaseTrait};

const TX_DESC_NUM: u32 = 32;
const TX_DESC_SIZE: u32 = TX_DESC_NUM * core::mem::size_of::<TxDescriptor>() as u32;

const RX_DESC_NUM: u32 = 32;
const RX_DESC_SIZE: u32 = RX_DESC_NUM * core::mem::size_of::<RxDescriptor>() as u32;

#[derive(Copy, Clone, Debug)]
enum Error {
    UnknownBar,
    NoEeprom,
    OutOfMemory,
    ReadErr(ReadErr),
}

impl From<ReadErr> for Error {
    fn from(value: ReadErr) -> Self {
        Self::ReadErr(value)
    }
}

#[derive(Copy, Clone)]
#[repr(usize)]
enum Register {
    Control = 0x0,
    Status = 0x8,

    Eeprom = 0x14,

    ICause = 0xc0,
    IRate = 0xc4,
    IMask = 0xd0,

    RCtrl = 0x100,
    /// Lower bits of the 64 bit descriptor base address.
    RxDescLo = 0x2800,
    /// Upper 32 bits of the 64 bit descriptor base address.
    RxDescHi = 0x2804,
    /// Descriptor length and must be 128B aligned.
    RxDescLen = 0x2808,
    /// Head pointer for the receive descriptor buffer.
    RxDescHead = 0x2810,
    /// Tail pointer for the receive descriptor buffer.
    RxDescTail = 0x2818,

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
    #[derive(Default)]
    struct TStatus: u8 {
        const DD = 1 << 0; // Descriptor Done
        const EC = 1 << 1; // Excess Collisions
        const LC = 1 << 2; // Late Collision
        const TU = 1 << 3; // Transmit Underrun
    }
}

bitflags::bitflags! {
    struct ECtl: u32 {
        const LRST    = 1 << 3;
        const ASDE    = 1 << 5;
        const SLU     = 1 << 6; // Set Link Up
        const ILOS    = 1 << 7;
        const RST     = 1 << 26;
        const VME     = 1 << 30;
        const PHY_RST = 1 << 31;
    }
}

bitflags::bitflags! {
    struct TCtl: u32 {
        const EN     = 1 << 1;  // Transmit Enable
        const PSP    = 1 << 3;  // Pad Short Packets
        const SWXOFF = 1 << 22; // Software XOFF Transmission
        const RTLC   = 1 << 24; // Re-transmit on Late Collision
    }
}

impl TCtl {
    /// Sets the number of attempts at retransmission prior to giving
    /// up on the packet (not including the first transmission attempt).
    fn set_collision_threshold(&mut self, value: u8) {
        self.bits |= (value as u32) << 4;
    }

    /// Sets the minimum number of byte times which must elapse for
    /// proper CSMA/CD operation.
    fn set_collision_distance(&mut self, value: u8) {
        self.bits |= (value as u32) << 12;
    }
}

bitflags::bitflags! {
    struct RCtl: u32 {
        const EN            = 1 << 1;  // Receiver Enable
        const SBP           = 1 << 2;  // Store Bad Packets
        const UPE           = 1 << 3;  // Unicast Promiscuous Enabled
        const MPE           = 1 << 4;  // Multicast Promiscuous Enabled
        const LPE           = 1 << 5;  // Long Packet Reception Enable
        const LBM_NONE      = 0 << 6;  // No Loopback
        const LBM_PHY       = 3 << 6;  // PHY or external SerDesc loopback
        const RDMTS_HALF    = 0 << 8;  // Free Buffer Threshold is 1/2 of RDLEN
        const RDMTS_QUARTER = 1 << 8;  // Free Buffer Threshold is 1/4 of RDLEN
        const RDMTS_EIGHTH  = 2 << 8;  // Free Buffer Threshold is 1/8 of RDLEN
        const MO_36         = 0 << 12; // Multicast Offset - bits 47:36
        const MO_35         = 1 << 12; // Multicast Offset - bits 46:35
        const MO_34         = 2 << 12; // Multicast Offset - bits 45:34
        const MO_32         = 3 << 12; // Multicast Offset - bits 43:32
        const BAM           = 1 << 15; // Broadcast Accept Mode
        const VFE           = 1 << 18; // VLAN Filter Enable
        const CFIEN         = 1 << 19; // Canonical Form Indicator Enable
        const CFI           = 1 << 20; // Canonical Form Indicator Bit Value
        const DPF           = 1 << 22; // Discard Pause Frames
        const PMCF          = 1 << 23; // Pass MAC Control Frames
        const SECRC         = 1 << 26; // Strip Ethernet CRC

        // Receive Buffer Size - bits 17:16
        const BSIZE_256     = 3 << 16;
        const BSIZE_512     = 2 << 16;
        const BSIZE_1024    = 1 << 16;
        const BSIZE_2048    = 0 << 16;
        const BSIZE_4096    = (3 << 16) | (1 << 25);
        const BSIZE_8192    = (2 << 16) | (1 << 25);
        const BSIZE_16384   = (1 << 16) | (1 << 25);
    }
}

bitflags::bitflags! {
    pub struct InterruptFlags: u32 {
        const TXDW    = 1 << 0;  // Transmit Descriptor Written Back
        const TXQE    = 1 << 1;  // Transmit Queue Empty
        const LSC     = 1 << 2;  // Link Status Change
        const RXDMT0  = 1 << 4;  // Receive Descriptor Minimum Threshold
        const DSW     = 1 << 5;  // Disable SW Write Access
        const RXO     = 1 << 6;  // Receiver Overrun
        const RXT0    = 1 << 7;  // Receiver Timer Interrupt
        const MDAC    = 1 << 9;  // MDIO Access Complete
        const PHYINT  = 1 << 12; // PHY Interrupt
        const LSECPN  = 1 << 14; // MACsec Packet Number
        const TXD_LOW = 1 << 15; // Transmit Descriptor Low Threshold hit
        const SRPD    = 1 << 16; // Small Receive Packet Detected
        const ACK     = 1 << 17; // Receive ACK Frame Detected
        const ECCER   = 1 << 22; // ECC Error
    }
}

#[derive(Default)]
#[repr(C, packed)]
struct TxDescriptor {
    pub addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: TStatus,
    pub css: u8,
    pub special: u16,
}

#[derive(Default)]
#[repr(C, packed)]
struct RxDescriptor {
    pub addr: u64,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
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
    mac: MacAddr,

    tx_cur: usize,
    tx_ring: VirtAddr,

    rx_cur: usize,
    rx_ring: VirtAddr,
}

impl E1000 {
    fn new(header: &PciHeader) -> Result<Self, Error> {
        header.enable_bus_mastering();
        header.enable_mmio();

        let bar0 = header.get_bar(0).ok_or(Error::UnknownBar)?;

        let registers_addr = match bar0 {
            Bar::Memory64 { address, .. } => PhysAddr::new(address),
            Bar::Memory32 { address, .. } => PhysAddr::new(address as u64),
            _ => return Err(Error::UnknownBar),
        };

        let mut this = Self {
            base: registers_addr.as_hhdm_virt(),
            mac: MacAddr([0; 6]),

            tx_cur: 0,
            tx_ring: VirtAddr::zero(),

            rx_cur: 0,
            rx_ring: VirtAddr::zero(),
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

        this.mac = MacAddr(mac);

        this.init_tx()?;
        this.init_rx()?;

        // XXX: The e1000 does not support MSIx and MSI.
        let gsi = aml::get_subsystem().pci_route_pin(
            0,
            header.bus(),
            header.device(),
            header.function(),
            header.interrupt_pin(),
        );

        let vector = interrupts::allocate_vector();
        interrupts::register_handler(vector, irq_handler);

        crate::arch::apic::io_apic_setup_legacy_irq(gsi, vector, 0);

        // Clear statistical counters.
        for i in 0..128 {
            unsafe {
                this.write_raw(0x5200 + i * 4, 0);
            }
        }

        // Enable interrupts!
        this.write(
            Register::IMask,
            (InterruptFlags::TXDW
                | InterruptFlags::TXQE
                | InterruptFlags::LSC
                | InterruptFlags::RXDMT0
                | InterruptFlags::DSW
                | InterruptFlags::RXO
                | InterruptFlags::RXT0
                | InterruptFlags::MDAC
                | InterruptFlags::PHYINT
                | InterruptFlags::LSECPN
                | InterruptFlags::TXD_LOW
                | InterruptFlags::SRPD
                | InterruptFlags::ACK
                | InterruptFlags::ECCER)
                .bits(),
        );
        this.read(Register::ICause);

        this.link_up();

        log::trace!("e1000: successfully initialized");
        Ok(this)
    }

    fn handle_irq(&mut self) {
        let cause = self.read(Register::ICause);
        log::debug!("cause: {cause}");
        if cause & 0x80 != 0x80 {
            return;
        }

        let idx = self.rx_cur;
        let descriptor = &self.rx_ring()[idx];
        log::debug!("{}", descriptor.status);

        if descriptor.status & 0x1 == 0x1 {
            // we got some packies right here mate. lets notify the boyz
            DEVICE.get().unwrap().wq.notify_complete();
        }
    }

    fn send(&mut self, packet: net::Packet<net::Eth>) {
        let cur = self.tx_cur;
        let ring = self.tx_ring();

        ring[cur].addr = unsafe { packet.addr() - crate::PHYSICAL_MEMORY_OFFSET };
        ring[cur].length = packet.len() as _;
        ring[cur].cmd = 0b1011;
        ring[cur].status = TStatus::empty();

        drop(ring);
        self.tx_cur = (self.tx_cur + 1) % TX_DESC_NUM as usize;

        self.write(Register::TxDescTail, self.tx_cur as u32);
    }

    fn recv(&mut self) -> Option<net::RecvPacket> {
        let id = self.rx_cur;
        let desc = &mut self.rx_ring()[id];

        if desc.status & 0x1 != 0x1 {
            return None;
        }

        Some(net::RecvPacket {
            packet: net::Packet::<ethernet::Eth>::new(
                PhysAddr::new(desc.addr).as_hhdm_virt(),
                desc.length as usize,
            ),
            id,
        })
    }

    fn recv_end(&mut self, id: usize) {
        let desc = &mut self.rx_ring()[id];

        if desc.status & 0x1 != 0x1 {
            unreachable!()
        }

        desc.status = 0;

        let old = self.rx_cur;
        self.rx_cur = (self.rx_cur + 1) % RX_DESC_NUM as usize;
        self.write(Register::RxDescTail, old as u32);
    }

    fn link_up(&self) {
        self.insert_flags(Register::Control, ECtl::SLU.bits());

        while self.read(Register::Status) & 2 != 2 {
            core::hint::spin_loop();
        }
    }

    fn rx_ring(&mut self) -> &mut [RxDescriptor] {
        self.rx_ring
            .read_mut::<[RxDescriptor; RX_DESC_NUM as usize]>()
            .unwrap()
    }

    fn tx_ring(&mut self) -> &mut [TxDescriptor] {
        self.tx_ring
            .read_mut::<[TxDescriptor; TX_DESC_NUM as usize]>()
            .unwrap()
    }

    fn init_tx(&mut self) -> Result<(), Error> {
        assert!(TX_DESC_SIZE < Size4KiB::SIZE as u32);

        let frame: PhysFrame<Size4KiB> =
            FRAME_ALLOCATOR.allocate_frame().ok_or(Error::OutOfMemory)?;

        let phys = frame.start_address();
        let addr = phys.as_hhdm_virt();

        let descriptors = addr.read_mut::<[TxDescriptor; TX_DESC_NUM as usize]>()?;

        for desc in descriptors {
            *desc = TxDescriptor::default();
            desc.status = TStatus::DD;
        }

        self.tx_ring = addr;

        self.write(Register::TxDesLo, phys.as_u64() as _);
        self.write(Register::TxDesHi, (phys.as_u64() >> 32) as _);
        self.write(Register::TxDescLen, TX_DESC_SIZE);
        self.write(Register::TxDescHead, 0);
        self.write(Register::TxDescTail, 0);

        let mut flags = TCtl { bits: 1 << 28 } | TCtl::EN | TCtl::PSP | TCtl::RTLC;
        flags.set_collision_distance(64);
        flags.set_collision_threshold(15);

        self.write(Register::TCtrl, flags.bits());

        // TODO: Set the default values for the Tx Inter Packet
        //       Gap Timer.
        // self.write(Register::Tipg, 0x???????);

        Ok(())
    }

    fn init_rx(&mut self) -> Result<(), Error> {
        assert!(TX_DESC_SIZE < Size4KiB::SIZE as u32);

        let frame: PhysFrame<Size4KiB> =
            FRAME_ALLOCATOR.allocate_frame().ok_or(Error::OutOfMemory)?;

        let phys = frame.start_address();
        let addr = phys.as_hhdm_virt();

        let descriptors = addr.read_mut::<[RxDescriptor; RX_DESC_NUM as usize]>()?;

        for desc in descriptors {
            let frame: PhysFrame<Size4KiB> =
                FRAME_ALLOCATOR.allocate_frame().ok_or(Error::OutOfMemory)?;

            *desc = RxDescriptor::default();
            desc.addr = frame.start_address().as_u64();
        }

        self.rx_ring = addr;

        self.write(Register::RxDescLo, phys.as_u64() as _);
        self.write(Register::RxDescHi, (phys.as_u64() >> 32) as _);
        self.write(Register::RxDescLen, RX_DESC_SIZE);
        self.write(Register::RxDescHead, 0);
        self.write(Register::RxDescTail, RX_DESC_NUM - 1);

        let flags = RCtl::EN
            | RCtl::UPE
            | RCtl::LPE
            | RCtl::MPE
            | RCtl::LBM_NONE
            | RCtl::RDMTS_EIGHTH
            | RCtl::BAM
            | RCtl::SECRC
            | RCtl::BSIZE_4096;

        self.write(Register::RCtrl, flags.bits());
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
        unsafe { self.write_raw(register as u32, value) }
    }

    fn read(&self, register: Register) -> u32 {
        unsafe { self.read_raw(register as u32) }
    }

    unsafe fn write_raw(&self, register: u32, value: u32) {
        unsafe {
            let register = self.base.as_mut_ptr::<u8>().add(register as usize);
            core::ptr::write_volatile(register as *mut u32, value);
        }
    }

    unsafe fn read_raw(&self, register: u32) -> u32 {
        let register = self.base.as_ptr::<u8>().add(register as usize);
        core::ptr::read_volatile(register as *const u32)
    }
}

struct Device {
    e1000: Mutex<E1000>,
    wq: BlockQueue,
}

impl Device {
    fn new(e1000: E1000) -> Self {
        Self {
            e1000: Mutex::new(e1000),
            wq: BlockQueue::new(),
        }
    }

    fn handle_irq(&self) {
        self.e1000.lock_irq().handle_irq()
    }
}

impl NetworkDriver for Device {
    fn send(&self, packet: net::Packet<net::Eth>) {
        self.e1000.lock_irq().send(packet)
    }

    fn recv(&self) -> net::RecvPacket {
        let task = scheduler::get_scheduler().current_task();
        self.wq.insert(task.clone());

        loop {
            let mut e1000 = self.e1000.lock_irq();
            if let Some(data) = e1000.recv() {
                self.wq.remove(task.clone());
                return data;
            } else {
                drop(e1000);
                scheduler::get_scheduler().inner.await_io().unwrap();
            }
        }
    }

    fn recv_end(&self, packet_id: usize) {
        self.e1000.lock_irq().recv_end(packet_id)
    }

    fn mac(&self) -> net::MacAddr {
        self.e1000.lock_irq().mac
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
        let e1000 = E1000::new(header).unwrap();
        let device = Arc::new(Device::new(e1000));

        DEVICE.call_once(|| device.clone());
        net::add_device(NetworkDevice::new(device));
    }
}

static DEVICE: Once<Arc<Device>> = Once::new();

fn irq_handler(_stack: &mut InterruptStack) {
    DEVICE.get().map(|e| e.handle_irq());
}

fn init() {
    register_device_driver(Handler::new())
}

crate::module_init!(init, ModuleType::Block);
