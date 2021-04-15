use crate::utils::io;
use crate::{arch::memory::paging::GlobalAllocator, drivers::ahci::AHCI};

use bit_field::BitField;
use x86_64::structures::paging::OffsetPageTable;

pub const PCI_CONFIG_ADDRESS_PORT: u16 = 0xCF8;
pub const PCI_CONFIG_DATA_PORT: u16 = 0xCFC;

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

    /*
     * Base Class 0x00 - Devices that predate Class Codes
     */
    LegacyVgaCompatible,
    LegacyNotVgaCompatible,

    /*
     * Base Class 0x01 - Mass Storage Controllers
     */
    ScsiBusController,
    IdeController,
    FloppyController,
    IpiBusController,
    RaidController,
    AtaController,
    SataController,
    SasController,
    OtherMassStorageController,

    /*
     * Base Class 0x02 - Network Controllers
     */
    EthernetController,
    TokenRingController,
    FddiController,
    AtmController,
    IsdnController,
    PicmgController,
    OtherNetworkController,

    /*
     * Base Class 0x03 - Display Controllers
     */
    VgaCompatibleController,
    XgaController,
    ThreeDController,
    OtherDisplayController,

    /*
     * Base Class 0x04 - Multimedia Devices
     */
    VideoDevice,
    AudioDevice,
    TelephonyDevice,
    OtherMultimediaDevice,

    /*
     * Base Class 0x05 - Memory Controllers
     */
    RamController,
    FlashController,
    OtherMemoryController,

    /*
     * Base Class 0x06 - Bridge Devices
     */
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

    /*
     * Base Class 0x07 - Simple Communications Controllers
     */
    SerialController,
    ParallelPort,
    MultiportSerialController,
    Modem,
    GpibController,
    SmartCard,
    OtherCommunicationsDevice,

    /*
     * Base Class 0x08 - Generic System Peripherals
     */
    InterruptController,
    DmaController,
    SystemTimer,
    RtcController,
    GenericPciHotPlugController,
    SdHostController,
    OtherSystemPeripheral,

    /*
     * Base Class 0x09 - Input Devices
     */
    KeyboardController,
    Digitizer,
    MouseController,
    ScannerController,
    GameportController,
    OtherInputController,

    /*
     * Base Class 0x0a - Docking Stations
     */
    GenericDockingStation,
    OtherDockingStation,

    /*
     * Base Class 0x0b - Processors
     */
    Processor386,
    Processor486,
    ProcessorPentium,
    ProcessorAlpha,
    ProcessorPowerPc,
    ProcessorMips,
    CoProcessor,

    /*
     * Base Class 0x0c - Serial Bus Controllers
     */
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

    /*
     * Base Class 0x0d - Wireless Controllers
     */
    IrdaController,
    ConsumerIrController,
    RfController,
    BluetoothController,
    BroadbandController,
    Ethernet5GHzController,
    Ethernet24GHzController,
    OtherWirelessController,

    /*
     * Base Class 0x0e - Intelligent IO Controllers
     */
    IntelligentIoController,

    /*
     * Base Class 0x0f - Satellite Communications Controllers
     */
    TvSatelliteCommunicationsController,
    AudioSatelliteCommunicationsController,
    VoiceSatelliteCommunicationsController,
    DataSatelliteCommunicationsController,

    /*
     * Base Class 0x10 - Encryption and Decryption Controllers
     */
    NetworkCryptionController,
    EntertainmentCryptionController,
    OtherCryptionController,

    /*
     * Base Class 0x11 - Data Acquisition and Signal Processing Controllers
     */
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
    AMD,
    NVIDIA,
    Qemu,
    Unknown(u32),
}

impl Vendor {
    pub fn new(id: u32) -> Self {
        match id {
            0x8086 => Self::Intel,
            0x1022 => Self::AMD,
            0x10DE => Self::NVIDIA,
            0x1234 => Self::Qemu,
            _ => Self::Unknown(id),
        }
    }
}

pub struct PCIHeader(u32);

impl PCIHeader {
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        let mut result: u32 = 0;

        result.set_bits(0..3, function as u32);
        result.set_bits(3..8, device as u32);
        result.set_bits(8..16, bus as u32);
        result.set_bits(16..32, 0);

        Self(result)
    }

    #[inline]
    pub fn bus(&self) -> u8 {
        self.0.get_bits(8..16) as u8
    }

    #[inline]
    pub fn device(&self) -> u8 {
        self.0.get_bits(3..8) as u8
    }

    #[inline]
    pub fn function(&self) -> u8 {
        self.0.get_bits(0..3) as u8
    }

    unsafe fn read(&self, offset: u32) -> u32 {
        let bus = self.bus() as u32;
        let device = self.device() as u32;
        let func = self.function() as u32;
        let offset = offset as u32;

        let address =
            ((bus << 16) | (device << 11) | (func << 8) | (offset & 0xFC) | 0x80000000) as u32;

        io::outl(PCI_CONFIG_ADDRESS_PORT, address);
        io::inl(PCI_CONFIG_DATA_PORT)
    }

    unsafe fn write(&self, offset: u32, value: u32) {
        let bus = self.bus() as u32;
        let device = self.device() as u32;
        let func = self.function() as u32;
        let offset = offset as u32;

        let address =
            ((bus << 16) | (device << 11) | (func << 8) | (offset & 0xFC) | 0x80000000) as u32;

        io::outl(PCI_CONFIG_ADDRESS_PORT, address);
        io::outl(PCI_CONFIG_DATA_PORT, value);
    }

    pub unsafe fn get_vendor_id(&self) -> u32 {
        let id = self.read(0x00);

        id.get_bits(0..16)
    }

    pub unsafe fn get_interface_id(&self) -> u32 {
        let id = self.read(0x08);

        id.get_bits(8..16)
    }

    #[inline]
    pub fn get_vendor(&self) -> Vendor {
        Vendor::new(unsafe { self.get_vendor_id() })
    }

    pub unsafe fn get_device(&self) -> DeviceType {
        let id = self.read(0x08);

        DeviceType::new(id.get_bits(24..32), id.get_bits(16..24))
    }

    #[inline]
    pub fn has_multiple_functions(&self) -> bool {
        unsafe { self.read(0x0c) }.get_bit(23)
    }

    pub unsafe fn get_bar(&self, bar: u8) -> Option<Bar> {
        let offset = 0x10 + (bar as u16) * 4;
        let bar = self.read(offset.into());

        if bar.get_bit(0) == false {
            let prefetchable = bar.get_bit(3);
            let address = bar.get_bits(4..32) << 4;

            self.write(offset.into(), 0xFFFFFFFF);

            let mut readback = self.read(offset.into());

            self.write(offset.into(), address);

            if readback == 0x0 {
                return None;
            }

            readback.set_bits(0..4, 0);

            let size = 1 << readback.trailing_zeros();

            match bar.get_bits(1..3) {
                0b00 => Some(Bar::Memory32 {
                    address,
                    size,
                    prefetchable,
                }),

                0b10 => {
                    let address = {
                        let mut address = address as u64;

                        address.set_bits(32..64, self.read((offset + 4).into()) as u64);
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
        } else {
            Some(Bar::IO(bar.get_bits(2..32)))
        }
    }
}

/// Lookup and initialize all PCI devices.
pub fn init(offset_table: &mut OffsetPageTable, frame_allocator: &mut GlobalAllocator) {
    // Use the brute force method to go through each possible bus,
    // device, function ID and check if we have a driver for it. If a driver
    // for the PCI device is found then initialize it.

    for bus in 0..255 {
        for device in 0..32 {
            let function_count = if PCIHeader::new(bus, device, 0x00).has_multiple_functions() {
                8
            } else {
                1
            };

            for function in 0..function_count {
                let device = PCIHeader::new(bus, device, function);

                unsafe {
                    if device.get_vendor_id() == 0xFFFF {
                        // Device does not exist.
                        continue;
                    }

                    log::info!(
                        "PCI device (device={:?}, vendor={:?})",
                        device.get_device(),
                        device.get_vendor()
                    );

                    match device.get_device() {
                        DeviceType::SataController => match device.get_interface_id() {
                            0x01 => {
                                /*
                                 * AHCI 1.0 Device
                                 */

                                AHCI::new(offset_table, frame_allocator, device);
                            }
                            _ => (),
                        },
                        _ => (),
                    }
                }
            }
        }
    }
}
