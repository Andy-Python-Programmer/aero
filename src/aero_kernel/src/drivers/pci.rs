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

use alloc::alloc::Global;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::utils::bitmap::Bitmap;
use crate::utils::sync::Mutex;

use crate::acpi::mcfg;
use crate::mem::paging::{OffsetPageTable, PhysAddr};
use crate::utils::VolatileCell;

use crate::arch::{apic, io};

use bit_field::BitField;

static PCI_TABLE: Mutex<PciTable> = Mutex::new(PciTable::new());

const PCI_CONFIG_ADDRESS_PORT: u16 = 0xCF8;
const PCI_CONFIG_DATA_PORT: u16 = 0xCFC;

bitflags::bitflags! {
    pub struct ProgramInterface: u8 {
        const PRIMARY_PCI_NATIVE   = 0b00000001;
        const PRIMARY_CAN_SWITCH   = 0b00000010;
        const SECONDARY_PCI_NATIVE = 0b00000100;
        const SECONDARY_CAN_SWITCH = 0b00001000;
        const DMA_CAPABLE          = 0b10000000;
    }
}

#[repr(u32)]
pub enum DeliveryMode {
    /// Deliver the signal to all the agents listed in the destination. The Trigger Mode for
    /// fixed delivery mode can be edge or level.
    Fixed = 0b000,
}

#[repr(C)]
struct Message {
    addr_lower: VolatileCell<u32>,
    addr_upper: VolatileCell<u32>,
    data: VolatileCell<u32>,
    mask: VolatileCell<u32>,
}

impl Message {
    fn is_masked(&self) -> bool {
        self.mask.get().get_bit(0)
    }

    fn set_masked(&self, masked: bool) {
        self.mask.set(*self.mask.get().set_bit(0, masked));
        self.mask.set(*self.mask.get().set_bit(30, masked));
    }

    fn set(&mut self, vector: u8, delivery_mode: DeliveryMode) {
        assert!(self.is_masked(), "msix: message is unmasked");

        let mut data = 0;
        data.set_bits(0..8, vector as u32);
        data.set_bits(8..11, delivery_mode as u32);
        data.set_bit(14, false);
        data.set_bit(15, false);
        data.set_bits(16..32, 0);

        let mut addr = 0;
        addr.set_bits(12..20, apic::get_bsp_id() as u32);
        addr.set_bits(20..32, 0xfee);

        self.data.set(data);
        self.addr_lower.set(addr);
        self.addr_upper.set(0);
    }
}

pub struct Msix<'a> {
    messages: &'a mut [Message],
    table: Bitmap<Global>,
}

impl<'a> Msix<'a> {
    pub fn new(header: &'a PciHeader, offset: u32) -> Self {
        let mut message_control = unsafe { header.read::<u16>(offset + 2) } as u16;

        // 31             16 15           8 7             0
        // ------------------------------------------------
        // Message Control | Next Pointer | Capability ID |
        // -----------------------------------------------
        //
        // XXX: table length is encoded as N - 1, so we add one to get N.
        let table_length = message_control.get_bits(0..11) + 1;

        let table_ptr = unsafe { header.read::<u32>(offset + 4) };
        // BAR index specifies the BAR number whose address range contains the MSI-X Table.
        let bar_index = table_ptr.get_bits(0..3) as u8;
        let bar_offset = table_ptr & !0b111;

        let bar = header
            .get_bar(bar_index)
            .expect("msix: table bar not present");

        map_bar(&bar);

        let bar_address = match bar {
            Bar::Memory64 { address, .. } => PhysAddr::new(address),
            Bar::Memory32 { address, .. } => PhysAddr::new(address as u64),
            _ => unreachable!(),
        };

        // SAFETY: We have exclusive access to the BAR and the slice is in bounds.
        let messages = unsafe {
            core::slice::from_raw_parts_mut(
                (bar_address.as_hhdm_virt() + bar_offset as u64).as_mut_ptr::<Message>(),
                table_length as usize,
            )
        };

        unsafe {
            message_control.set_bit(15, true); // enable MSI-X
            message_control.set_bit(14, false); // function mask

            header.disable_legacy_irq();
            header.write::<u16>(offset + 2, message_control as u32);
        }

        Self {
            messages,
            table: Bitmap::new_in(Global, table_length as usize),
        }
    }

    pub fn set(&mut self, vector: u8) -> usize {
        let msix_vector = self
            .table
            .find_first_unset()
            .expect("msix: no free vectors");

        self.table.set(msix_vector, true);

        let message = &mut self.messages[msix_vector];
        message.set(vector, DeliveryMode::Fixed);
        message.set_masked(false);

        msix_vector
    }
}

#[derive(PartialEq, Debug)]
pub enum Capability {
    Msi,
    Msix,

    Unknown,
}

pub struct CapabilityIter<'a> {
    offset: u32,
    header: &'a PciHeader,
}

impl<'a> CapabilityIter<'a> {
    fn new(device: &'a PciHeader, offset: u32) -> Self {
        Self {
            offset,
            header: device,
        }
    }
}

impl Iterator for CapabilityIter<'_> {
    type Item = (u32, Capability);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == 0 {
            return None;
        }

        // Parse the capabilities linked-list.
        //
        // 15           8 7             0
        // ------------------------------
        // Next Pointer | Capability ID |
        // ------------------------------
        let id = unsafe { self.header.read::<u8>(self.offset) };
        let capability = match id {
            0x5 => Capability::Msi,
            0x11 => Capability::Msix,

            _ => Capability::Unknown,
        };

        let old_offset = self.offset;
        self.offset = unsafe { self.header.read::<u8>(self.offset + 1) };

        Some((old_offset, capability))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Bar {
    Memory32 {
        address: u32,
        size: u32,
        prefetchable: bool,
    },

    Memory64 {
        address: u64,
        size: u64,
        prefetchable: bool,
    },

    IO(u32),
}

#[derive(Debug, PartialEq)]
pub enum DeviceType {
    Unknown,

    // Base Class 0x00 - Devices that predate Class Codes
    LegacyVgaCompatible,
    LegacyNotVgaCompatible,

    // Base Class 0x01 - Mass Storage Controllers
    ScsiBusController,
    IdeController,
    FloppyController,
    IpiBusController,
    RaidController,
    AtaController,
    SataController,
    SasController,
    NvmeController,
    OtherMassStorageController,

    // Base Class 0x02 - Network Controllers
    EthernetController,
    TokenRingController,
    FddiController,
    AtmController,
    IsdnController,
    PicmgController,
    OtherNetworkController,

    // Base Class 0x03 - Display Controllers
    VgaCompatibleController,
    XgaController,
    ThreeDController,
    OtherDisplayController,

    // Base Class 0x04 - Multimedia Devices
    VideoDevice,
    AudioDevice,
    TelephonyDevice,
    OtherMultimediaDevice,

    // Base Class 0x05 - Memory Controllers
    RamController,
    FlashController,
    OtherMemoryController,

    // Base Class 0x06 - Bridge Devices
    HostBridge,
    IsaBridge,
    EisaBridge,
    McaBridge,
    PciPciBridge,
    PcmciaBridge,
    NuBusBridge,
    CardBusBridge,
    RacewayBridge,
    SemiTransparentPciPciBridge,
    InfinibandPciHostBridge,
    OtherBridgeDevice,

    // Base Class 0x07 - Simple Communications Controllers
    SerialController,
    ParallelPort,
    MultiportSerialController,
    Modem,
    GpibController,
    SmartCard,
    OtherCommunicationsDevice,

    // Base Class 0x08 - Generic System Peripherals
    InterruptController,
    DmaController,
    SystemTimer,
    RtcController,
    GenericPciHotPlugController,
    SdHostController,
    OtherSystemPeripheral,

    // Base Class 0x09 - Input Devices
    KeyboardController,
    Digitizer,
    MouseController,
    ScannerController,
    GameportController,
    OtherInputController,

    // Base Class 0x0a - Docking Stations
    GenericDockingStation,
    OtherDockingStation,

    // Base Class 0x0b - Processors
    Processor386,
    Processor486,
    ProcessorPentium,
    ProcessorAlpha,
    ProcessorPowerPc,
    ProcessorMips,
    CoProcessor,

    // Base Class 0x0c - Serial Bus Controllers
    FirewireController,
    AccessBusController,
    SsaBusController,
    UsbController,
    FibreChannelController,
    SmBusController,
    InfiniBandController,
    IpmiController,
    SercosController,
    CanBusController,

    // Base Class 0x0d - Wireless Controllers
    IrdaController,
    ConsumerIrController,
    RfController,
    BluetoothController,
    BroadbandController,
    Ethernet5GHzController,
    Ethernet24GHzController,
    OtherWirelessController,

    // Base Class 0x0e - Intelligent IO Controllers
    IntelligentIoController,

    // Base Class 0x0f - Satellite Communications Controllers
    TvSatelliteCommunicationsController,
    AudioSatelliteCommunicationsController,
    VoiceSatelliteCommunicationsController,
    DataSatelliteCommunicationsController,

    // Base Class 0x10 - Encryption and Decryption Controllers
    NetworkCryptionController,
    EntertainmentCryptionController,
    OtherCryptionController,

    // Base Class 0x11 - Data Acquisition and Signal Processing Controllers
    DpioModule,
    PerformanceCounter,
    CommunicationsSynchronizationController,
    ManagementCard,
    OtherSignalProcessingController,
}

impl DeviceType {
    pub fn new(base_class: u32, sub_class: u32) -> Self {
        match (base_class, sub_class) {
            (0x00, 0x00) => DeviceType::LegacyNotVgaCompatible,
            (0x00, 0x01) => DeviceType::LegacyVgaCompatible,

            (0x01, 0x00) => DeviceType::ScsiBusController,
            (0x01, 0x01) => DeviceType::IdeController,
            (0x01, 0x02) => DeviceType::FloppyController,
            (0x01, 0x03) => DeviceType::IpiBusController,
            (0x01, 0x04) => DeviceType::RaidController,
            (0x01, 0x05) => DeviceType::AtaController,
            (0x01, 0x06) => DeviceType::SataController,
            (0x01, 0x07) => DeviceType::SasController,
            (0x01, 0x08) => DeviceType::NvmeController,
            (0x01, 0x80) => DeviceType::OtherMassStorageController,

            (0x02, 0x00) => DeviceType::EthernetController,
            (0x02, 0x01) => DeviceType::TokenRingController,
            (0x02, 0x02) => DeviceType::FddiController,
            (0x02, 0x03) => DeviceType::AtmController,
            (0x02, 0x04) => DeviceType::IsdnController,
            (0x02, 0x06) => DeviceType::PicmgController,
            (0x02, 0x80) => DeviceType::OtherNetworkController,

            (0x03, 0x00) => DeviceType::VgaCompatibleController,
            (0x03, 0x01) => DeviceType::XgaController,
            (0x03, 0x02) => DeviceType::ThreeDController,
            (0x03, 0x80) => DeviceType::OtherDisplayController,

            (0x04, 0x00) => DeviceType::VideoDevice,
            (0x04, 0x01) => DeviceType::AudioDevice,
            (0x04, 0x02) => DeviceType::TelephonyDevice,
            (0x04, 0x03) => DeviceType::OtherMultimediaDevice,

            (0x05, 0x00) => DeviceType::RamController,
            (0x05, 0x01) => DeviceType::FlashController,
            (0x05, 0x02) => DeviceType::OtherMemoryController,

            (0x06, 0x00) => DeviceType::HostBridge,
            (0x06, 0x01) => DeviceType::IsaBridge,
            (0x06, 0x02) => DeviceType::EisaBridge,
            (0x06, 0x03) => DeviceType::McaBridge,
            (0x06, 0x04) => DeviceType::PciPciBridge,
            (0x06, 0x05) => DeviceType::PcmciaBridge,
            (0x06, 0x06) => DeviceType::NuBusBridge,
            (0x06, 0x07) => DeviceType::CardBusBridge,
            (0x06, 0x08) => DeviceType::RacewayBridge,
            (0x06, 0x09) => DeviceType::SemiTransparentPciPciBridge,
            (0x06, 0x0a) => DeviceType::InfinibandPciHostBridge,
            (0x06, 0x80) => DeviceType::OtherBridgeDevice,

            (0x07, 0x00) => DeviceType::SerialController,
            (0x07, 0x01) => DeviceType::ParallelPort,
            (0x07, 0x02) => DeviceType::MultiportSerialController,
            (0x07, 0x03) => DeviceType::Modem,
            (0x07, 0x04) => DeviceType::GpibController,
            (0x07, 0x05) => DeviceType::SmartCard,
            (0x07, 0x80) => DeviceType::OtherCommunicationsDevice,

            (0x08, 0x00) => DeviceType::InterruptController,
            (0x08, 0x01) => DeviceType::DmaController,
            (0x08, 0x02) => DeviceType::SystemTimer,
            (0x08, 0x03) => DeviceType::RtcController,
            (0x08, 0x04) => DeviceType::GenericPciHotPlugController,
            (0x08, 0x05) => DeviceType::SdHostController,
            (0x08, 0x80) => DeviceType::OtherSystemPeripheral,

            (0x09, 0x00) => DeviceType::KeyboardController,
            (0x09, 0x01) => DeviceType::Digitizer,
            (0x09, 0x02) => DeviceType::MouseController,
            (0x09, 0x03) => DeviceType::ScannerController,
            (0x09, 0x04) => DeviceType::GameportController,
            (0x09, 0x80) => DeviceType::OtherInputController,

            (0x0a, 0x00) => DeviceType::GenericDockingStation,
            (0x0a, 0x80) => DeviceType::OtherDockingStation,

            (0x0b, 0x00) => DeviceType::Processor386,
            (0x0b, 0x01) => DeviceType::Processor486,
            (0x0b, 0x02) => DeviceType::ProcessorPentium,
            (0x0b, 0x10) => DeviceType::ProcessorAlpha,
            (0x0b, 0x20) => DeviceType::ProcessorPowerPc,
            (0x0b, 0x30) => DeviceType::ProcessorMips,
            (0x0b, 0x40) => DeviceType::CoProcessor,

            (0x0c, 0x00) => DeviceType::FirewireController,
            (0x0c, 0x01) => DeviceType::AccessBusController,
            (0x0c, 0x02) => DeviceType::SsaBusController,
            (0x0c, 0x03) => DeviceType::UsbController,
            (0x0c, 0x04) => DeviceType::FibreChannelController,
            (0x0c, 0x05) => DeviceType::SmBusController,
            (0x0c, 0x06) => DeviceType::InfiniBandController,
            (0x0c, 0x07) => DeviceType::IpmiController,
            (0x0c, 0x08) => DeviceType::SercosController,
            (0x0c, 0x09) => DeviceType::CanBusController,

            (0x0d, 0x00) => DeviceType::IrdaController,
            (0x0d, 0x01) => DeviceType::ConsumerIrController,
            (0x0d, 0x10) => DeviceType::RfController,
            (0x0d, 0x11) => DeviceType::BluetoothController,
            (0x0d, 0x12) => DeviceType::BroadbandController,
            (0x0d, 0x20) => DeviceType::Ethernet5GHzController,
            (0x0d, 0x21) => DeviceType::Ethernet24GHzController,
            (0x0d, 0x80) => DeviceType::OtherWirelessController,

            (0x0e, 0x00) => DeviceType::IntelligentIoController,

            (0x0f, 0x00) => DeviceType::TvSatelliteCommunicationsController,
            (0x0f, 0x01) => DeviceType::AudioSatelliteCommunicationsController,
            (0x0f, 0x02) => DeviceType::VoiceSatelliteCommunicationsController,
            (0x0f, 0x03) => DeviceType::DataSatelliteCommunicationsController,

            (0x10, 0x00) => DeviceType::NetworkCryptionController,
            (0x10, 0x10) => DeviceType::EntertainmentCryptionController,
            (0x10, 0x80) => DeviceType::OtherCryptionController,

            (0x11, 0x00) => DeviceType::DpioModule,
            (0x11, 0x01) => DeviceType::PerformanceCounter,
            (0x11, 0x10) => DeviceType::CommunicationsSynchronizationController,
            (0x11, 0x20) => DeviceType::ManagementCard,
            (0x11, 0x80) => DeviceType::OtherSignalProcessingController,

            _ => DeviceType::Unknown,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Vendor {
    Intel,
    Amd,
    Nvidia,
    Qemu,
    Unknown(u32),
}

impl Vendor {
    pub fn new(id: u32) -> Self {
        match id {
            0x8086 => Self::Intel,
            0x1022 => Self::Amd,
            0x10DE => Self::Nvidia,
            0x1234 => Self::Qemu,
            _ => Self::Unknown(id),
        }
    }

    pub fn is_valid(&self) -> bool {
        match self {
            Self::Unknown(id) => *id != 0xFFFF,
            _ => true,
        }
    }
}

pub struct PciHeader(u32);

impl PciHeader {
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        let mut result: u32 = 0;

        result.set_bits(0..3, function as u32);
        result.set_bits(3..8, device as u32);
        result.set_bits(8..16, bus as u32);
        result.set_bits(16..32, 0);

        Self(result)
    }

    pub fn bus(&self) -> u8 {
        self.0.get_bits(8..16) as u8
    }

    pub fn device(&self) -> u8 {
        self.0.get_bits(3..8) as u8
    }

    pub fn function(&self) -> u8 {
        self.0.get_bits(0..3) as u8
    }

    pub unsafe fn read<T>(&self, offset: u32) -> u32 {
        let bus = self.bus() as u32;
        let device = self.device() as u32;
        let func = self.function() as u32;
        let address = (bus << 16) | (device << 11) | (func << 8) | (offset & 0xFC) | 0x80000000;

        io::outl(PCI_CONFIG_ADDRESS_PORT, address);

        let offset = (offset & 0b11) * 8;
        let val = io::inl(PCI_CONFIG_DATA_PORT);

        match core::mem::size_of::<T>() {
            1 => (val >> offset) as u8 as u32,  // u8
            2 => (val >> offset) as u16 as u32, // u16
            4 => val,                           // u32
            width => unreachable!("unknown PCI read width: `{}`", width),
        }
    }

    unsafe fn write<T>(&self, offset: u32, value: u32) {
        let current = self.read::<u32>(offset);

        let bus = self.bus() as u32;
        let device = self.device() as u32;
        let func = self.function() as u32;

        let address = (bus << 16) | (device << 11) | (func << 8) | (offset & 0xFC) | 0x80000000;
        let noffset = (offset & 0b11) * 8;

        io::outl(PCI_CONFIG_ADDRESS_PORT, address);
        match core::mem::size_of::<T>() {
            1 => {
                let mask = !(0xffu32 << offset);
                let value = (current & mask) | ((value & 0xff) << offset);
                io::outl(PCI_CONFIG_DATA_PORT, value)
            } // u8

            2 => {
                let mask = !(0xffffu32 << noffset);
                let value = (current & mask) | ((value & 0xffff) << noffset);
                io::outl(PCI_CONFIG_DATA_PORT, value)
            } // u16

            4 => io::outl(PCI_CONFIG_DATA_PORT, value), // u32
            width => unreachable!("unknown PCI write width: `{}`", width),
        }
    }

    /// Enables response to memory accesses on the primary interface that address a device
    /// that resides behind the bridge in both the memory mapped I/O and prefetchable memory
    /// ranges or targets a location within the bridge itself.
    pub fn enable_mmio(&self) {
        // Read the Command Register from the device's PCI Configuration Space, set bit 1
        // (MMIO bit) and write the modified Command Register.
        let command = unsafe { self.read::<u16>(0x04) };

        unsafe { self.write::<u16>(0x04, command | (1 << 1)) }
    }

    /// Enable the bridge to operate as a master on the primary interface for memory and I/O
    /// transactions forwarded from the secondary interface. This allows the PCI device to perform
    /// DMA.
    pub fn enable_bus_mastering(&self) {
        // Read the Command Register from the device's PCI Configuration Space, set bit 2
        // (bus mastering bit) and write the modified Command Register. Note that some BISOs do
        // enable bus mastering by default.
        let command = unsafe { self.read::<u16>(0x04) };

        unsafe { self.write::<u16>(0x04, command | (1 << 2)) }
    }

    pub fn disable_legacy_irq(&self) {
        // Set the Interrupt Disable bit, which is bit 10 of the Command register
        // (at Configuration Space offset 0x4) to disable legacy interrupts.
        let mut command = unsafe { self.read::<u16>(0x04) };
        command.set_bit(10, true);

        unsafe { self.write::<u16>(0x04, command) }
    }

    /// Returns the value stored in the PCI vendor ID register which is used to identify
    /// the manufacturer of the PCI device.
    pub fn get_vendor(&self) -> Vendor {
        unsafe { Vendor::new(self.read::<u16>(0x00)) }
    }

    pub unsafe fn get_device(&self) -> DeviceType {
        let id = self.read::<u32>(0x08);

        DeviceType::new(id.get_bits(24..32), id.get_bits(16..24))
    }

    pub fn has_multiple_functions(&self) -> bool {
        unsafe { self.read::<u32>(0x0c) }.get_bit(23)
    }

    /// Returns the value stored in the PCI header type register which is used to
    /// indicate layout for bytes,of the device’s configuration space.
    pub fn get_header_type(&self) -> u8 {
        unsafe { self.read::<u8>(0x0E) as u8 & 0b01111111 }
    }

    pub fn capabilities(&self) -> CapabilityIter {
        let offset = unsafe { self.read::<u8>(0x34) };
        CapabilityIter::new(self, offset)
    }

    pub fn msix(&self) -> Option<Msix> {
        self.capabilities()
            .find(|(_, e)| *e == Capability::Msix)
            .map(|(offset, _)| Msix::new(self, offset))
    }

    /// Returns the value stored in the bar of the provided slot. Returns [`None`] if the
    /// bar is empty.
    pub fn get_bar(&self, bar: u8) -> Option<Bar> {
        debug_assert!(self.get_header_type() == 0); // Ensure header type == 0
        debug_assert!(bar <= 5); // Make sure the bar is valid.

        let offset = 0x10 + (bar as u16) * 4;
        let bar = unsafe { self.read::<u32>(offset.into()) };

        // bit 0:true  - the BAR is in memory
        // bit 0:false - the BAR is in I/O
        if bar.get_bit(0) {
            Some(Bar::IO(bar.get_bits(2..32)))
        } else {
            let prefetchable = bar.get_bit(3);
            let address = bar.get_bits(4..32) << 4;

            let size = unsafe {
                self.write::<u32>(offset.into(), 0xffffffff);
                let mut readback = self.read::<u32>(offset.into());
                self.write::<u32>(offset.into(), address);

                // If the entire readback value is zero, the BAR is not implemented, so we
                // return `None`.
                if readback == 0x0 {
                    return None;
                }

                readback.set_bits(0..4, 0);
                1 << readback.trailing_zeros()
            };

            match bar.get_bits(1..3) {
                0b00 => Some(Bar::Memory32 {
                    address,
                    size,
                    prefetchable,
                }),

                0b10 => {
                    let address = {
                        let mut address = address as u64;

                        // Get the upper 32 bits of the address.
                        address.set_bits(
                            32..64,
                            unsafe { self.read::<u32>((offset + 4).into()) }.into(),
                        );

                        address
                    };

                    Some(Bar::Memory64 {
                        address,
                        size: size as u64,
                        prefetchable,
                    })
                }

                _ => None,
            }
        }
    }

    pub fn interrupt_pin(&self) -> u8 {
        unsafe { self.read::<u8>(0x3d) as u8 }
    }

    // NOTE: The Base Address registers are optional registers used to map internal
    // (device-specific) registers into Memory or I/O Spaces. Refer to the PCI Local Bus
    // Specification for a detailed discussion of base address registers.

    pub fn base_address0(&self) -> u32 {
        unsafe { self.read::<u32>(0x10) }
    }

    pub fn base_address1(&self) -> u32 {
        unsafe { self.read::<u32>(0x14) }
    }

    pub fn base_address2(&self) -> u32 {
        unsafe { self.read::<u32>(0x18) }
    }

    pub fn base_address3(&self) -> u32 {
        unsafe { self.read::<u32>(0x1C) }
    }

    pub fn base_address4(&self) -> u32 {
        unsafe { self.read::<u32>(0x20) }
    }

    pub fn program_interface(&self) -> ProgramInterface {
        let bits = unsafe { self.read::<u8>(0x09) };
        ProgramInterface::from_bits_truncate(bits as u8)
    }
}

pub trait PciDeviceHandle: Sync + Send {
    /// Returns true if the PCI device driver handles the device with
    /// the provided `vendor_id` and `device_id`.
    fn handles(&self, vendor_id: Vendor, device_id: DeviceType) -> bool;

    /// This function is responsible for initializing the device driver
    /// and starting it.
    fn start(&self, header: &PciHeader, offset_table: &mut OffsetPageTable);
}

struct PciDevice {
    handle: Arc<dyn PciDeviceHandle>,
}

struct PciTable {
    inner: Vec<PciDevice>,
}

impl PciTable {
    const fn new() -> Self {
        Self { inner: Vec::new() }
    }
}

pub fn map_bar(bar: &Bar) {
    use crate::mem::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB, UnmapError};

    use crate::mem::AddressSpace;

    let mut address_space = AddressSpace::this();
    let mut offset_table = address_space.offset_page_table();

    let (addr, size) = match bar {
        Bar::Memory64 { address, size, .. } => (PhysAddr::new(*address), *size),
        _ => unreachable!(),
    };

    for frame in PhysFrame::range(
        PhysFrame::<Size4KiB>::from_start_address(addr).unwrap(),
        PhysFrame::containing_address(addr + size),
    ) {
        let virt = frame.start_address().as_hhdm_virt();
        let page = Page::containing_address(virt);

        // Map will fail if the bar was partially mapped.
        match offset_table.unmap(page) {
            Ok((_, m)) => m.ignore(),
            Err(UnmapError::PageNotMapped) => {}
            Err(e) => unreachable!("{:?}", e),
        }

        unsafe {
            offset_table.map_to(
                page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            )
        }
        .unwrap()
        .flush();
    }
}

pub fn register_device_driver(handle: Arc<dyn PciDeviceHandle>) {
    PCI_TABLE.lock().inner.push(PciDevice { handle })
}

/// Lookup and initialize all PCI devices.
pub fn init(offset_table: &mut OffsetPageTable) {
    // Check if the MCFG table is available.
    if mcfg::is_available() {
        let mcfg_table = mcfg::get_mcfg_table();
        let _entry_count = mcfg_table.entry_count();
    }

    // Use the brute force method to go through each possible bus,
    // device, function ID and check if we have a driver for it. If a driver
    // for the PCI device is found then initialize it.
    for bus in 0..255 {
        for device in 0..32 {
            let function_count = if PciHeader::new(bus, device, 0x00).has_multiple_functions() {
                8
            } else {
                1
            };

            for function in 0..function_count {
                let device = PciHeader::new(bus, device, function);

                unsafe {
                    if !device.get_vendor().is_valid() {
                        // Device does not exist.
                        continue;
                    }

                    log::debug!(
                        "PCI device (device={:?}, vendor={:?})",
                        device.get_device(),
                        device.get_vendor()
                    );

                    for driver in &mut PCI_TABLE.lock().inner {
                        if driver
                            .handle
                            .handles(device.get_vendor(), device.get_device())
                        {
                            driver.handle.start(&device, offset_table)
                        }
                    }
                }
            }
        }
    }
}
