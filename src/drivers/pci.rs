use crate::utils::io;

pub const PCI_CONFIG_ADDRESS_PORT: u16 = 0xCF8;
pub const PCI_CONFIG_DATA_PORT: u16 = 0xCFC;

#[derive(Debug, PartialEq)]
pub enum DeviceClass {
    Unknown,
    MassStorageController,
    NetworkController,
    DisplayController,
    MultimediaController,
    MemoryController,
    BridgeDevice,
    SimpleCommunicationController,
    BaseSystemPeripheral,
    InputDeviceController,
    DockingStation,
    Processor,
    SerialBusController,
    WirelessController,
    IntelligentController,
    SatelliteCommunicationController,
    EncryptionController,
    UnassignedClass,
}

impl DeviceClass {
    pub fn from_u32(value: u32) -> Self {
        match value {
            0x00 => Self::Unknown,
            0x01 => Self::MassStorageController,
            0x02 => Self::NetworkController,
            0x03 => Self::DisplayController,
            0x04 => Self::MultimediaController,
            0x05 => Self::MemoryController,
            0x06 => Self::BridgeDevice,
            0x07 => Self::SimpleCommunicationController,
            0x08 => Self::BaseSystemPeripheral,
            0x09 => Self::InputDeviceController,
            0x0A => Self::DockingStation,
            0x0B => Self::Processor,
            0x0C => Self::SerialBusController,
            0x0D => Self::WirelessController,
            0x0E => Self::IntelligentController,
            0x0F => Self::SatelliteCommunicationController,
            0x10 => Self::EncryptionController,
            0xFF => Self::UnassignedClass,
            _ => panic!("{}", value),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PCIDevice {
    pub interrupt: u32,

    pub bus: u16,
    pub device: u16,
    pub function: u16,

    pub vendor_id: u32,
    pub device_id: u32,

    pub class_id: u32,
    pub subclass_id: u32,
    pub interface_id: u32,

    pub revision: u32,
}

impl PCIDevice {
    pub unsafe fn new(bus: u16, device: u16, function: u16) -> Self {
        let vendor_id = pci_read(bus, device, function, 0x00);
        let device_id = pci_read(bus, device, function, 0x02);

        let class_id = pci_read(bus, device, function, 0x0b);
        let subclass_id = pci_read(bus, device, function, 0x0A);
        let interface_id = pci_read(bus, device, function, 0x09);

        let revision = pci_read(bus, device, function, 0x08);
        let interrupt = pci_read(bus, device, function, 0x3C);

        Self {
            bus,
            device,
            function,
            vendor_id,
            device_id,
            class_id,
            subclass_id,
            interface_id,
            revision,
            interrupt,
        }
    }

    pub fn get_device_class(&self) -> DeviceClass {
        DeviceClass::from_u32(self.class_id)
    }
}

unsafe fn pci_read(bus: u16, device: u16, function: u16, register_offset: u32) -> u32 {
    let bus = bus as u32;
    let device = device as u32;
    let function = function as u32;

    let id = 0x1 << 31
        | ((bus & 0xFF) << 16)
        | ((device & 0x1F) << 11)
        | ((function & 0x07) << 8)
        | (register_offset & 0xFC);

    io::outl(PCI_CONFIG_ADDRESS_PORT, id);

    let result = io::inl(PCI_CONFIG_DATA_PORT);

    result >> (8 * (register_offset % 4))
}

pub struct PCI;

impl PCI {
    pub fn new() {
        for bus in 0..255 {
            for device in 0..32 {
                for function in 0..8 {
                    let device = unsafe { PCIDevice::new(bus, device, function) };

                    // Device doesn't exist
                    if device.vendor_id == 0xFFFF {
                        break;
                    }

                    if device.get_device_class() != DeviceClass::UnassignedClass {
                        crate::println!("{:?}", device.get_device_class())
                    }
                }
            }
        }
    }
}
