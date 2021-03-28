use core::mem::MaybeUninit;

use crate::{arch::memory::paging::GlobalAllocator, paging::memory_map_device};
use x86_64::{
    structures::paging::{OffsetPageTable, PhysFrame, Size4KiB},
    PhysAddr,
};

use super::pci::{Bar, PCIHeader};
use crate::log;

const SATA_SIG_ATAPI: u32 = 0xEB140101;
const SATA_SIG_ATA: u32 = 0x00000101;
const SATA_SIG_SEMB: u32 = 0xC33C0101;
const SATA_SIG_PM: u32 = 0x96690101;

const HBA_PORT_DEV_PRESENT: u32 = 0x3;
const HBA_PORT_IPM_ACTIVE: u32 = 0x1;

#[derive(Debug, PartialEq)]
pub enum AHCIPortType {
    None,
    SATA,
    SEMB,
    PM,
    SATAPI,
}

#[repr(C, packed)]
struct HBAMemory {
    host_capability: u32,
    global_host_control: u32,
    interrupt_status: u32,
    ports_implemented: u32,
    version: u32,
    ccc_control: u32,
    ccc_ports: u32,
    enclosure_management_location: u32,
    enclosure_management_control: u32,
    host_capabilities_extended: u32,
    bios_handoff_ctrl_sts: u32,
    rsv0: [u8; 0x74],
    vendor: [u8; 0x60],
    ports: [MaybeUninit<HBAPort>; 32],
}

impl HBAMemory {
    /// Get a HBA port at `idx`.
    pub fn get_port(&self, idx: u32) -> &HBAPort {
        assert!(idx < 32, "There are only 32 ports!");

        let bit = self.ports_implemented >> idx;

        if bit & 1 == 1 {
            let ptr = self.ports[idx as usize].as_ptr();

            unsafe { &*ptr }
        } else {
            panic!()
        }
    }
}

#[repr(C, packed)]
struct HBAPort {
    command_list_base: u32,
    command_list_base_upper: u32,
    fis_base_address: u32,
    fis_base_address_upper: u32,
    interrupt_status: u32,
    interrupt_enable: u32,
    cmd_sts: u32,
    rsv0: u32,
    task_file_data: u32,
    signature: u32,
    sata_status: u32,
    sata_control: u32,
    sata_error: u32,
    sata_active: u32,
    command_issue: u32,
    sata_notification: u32,
    fis_switch_control: u32,
    rsv1: [u32; 11],
    vendor: [u32; 4],
}

pub struct AHCI {
    header: PCIHeader,
    memory: &'static mut HBAMemory,
}

impl AHCI {
    pub unsafe fn new(
        offset_table: &mut OffsetPageTable,
        frame_allocator: &mut GlobalAllocator,
        header: PCIHeader,
    ) -> Self {
        log::info("Loaded AHCI driver");

        let abar = header.get_bar(5).unwrap();

        let (abar_address, abar_size) = match abar {
            Bar::Memory32 { address, size, .. } => (address as u64, size as u64),
            Bar::Memory64 { address, size, .. } => (address, size),
            Bar::IO { .. } => panic!("ABAR is in port space o_O"),
        };

        let start: PhysFrame<Size4KiB> =
            PhysFrame::containing_address(PhysAddr::new(abar_address as u64));
        let end =
            PhysFrame::containing_address(PhysAddr::new((abar_address + abar_size - 1) as u64));

        for frame in PhysFrame::range_inclusive(start, end) {
            memory_map_device(offset_table, frame_allocator, frame)
                .expect("Failed to memory map the SATA device");
        }

        let memory = &mut *(abar_address as *mut HBAMemory);
        let this = Self { header, memory };

        this.probe_ports();

        this
    }

    unsafe fn probe_ports(&self) {
        for i in 0..32 {
            if (self.memory.ports_implemented & (1 << i)) == 1 {
                let port = self.memory.get_port(i);
                let port_type = self.get_port_type(port);

                crate::println!("Found! {:?}{}", port_type, port.signature);
            }
        }
    }

    fn get_port_type(&self, port: &HBAPort) -> AHCIPortType {
        let ipm = (port.sata_status >> 8) & 0x0F;
        let device_detection = port.sata_status & 0x0F;

        if device_detection != HBA_PORT_DEV_PRESENT {
            return AHCIPortType::None;
        } else if ipm != HBA_PORT_IPM_ACTIVE {
            return AHCIPortType::None;
        }

        match port.signature {
            SATA_SIG_ATAPI => AHCIPortType::SATAPI,
            SATA_SIG_ATA => AHCIPortType::SATA,
            SATA_SIG_PM => AHCIPortType::PM,
            SATA_SIG_SEMB => AHCIPortType::SEMB,

            _ => AHCIPortType::None,
        }
    }
}
