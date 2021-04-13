use core::mem::MaybeUninit;

use crate::{arch::memory::paging::GlobalAllocator, paging::memory_map_device};
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use super::pci::{Bar, PCIHeader};

use bitflags::bitflags;

bitflags! {
    #[repr(C)]
    pub struct HBACapabilities: u32 {
        const SXS_SUPPORT = 1 << 5;
        const EMS_SUPPORT = 1 << 6;
        const CCC_SUPPORT = 1 << 7;
        const PS_CAPABLE = 1 << 13;
        const SS_CAPABLE = 1 << 14;
        const PIO_MULTI_DRQ_SUPPORT = 1 << 15;
        const FBSS_SUPPORT = 1 << 16;
        const PM_SUPPORT = 1 << 17;
        const AHCI_ONLY = 1 << 18;
        const CLO_SUPPORT = 1 << 24;
        const AL_SUPPORT = 1 << 25;
        const ALP_SUPPORT = 1 << 26;
        const SS_SUPPORT = 1 << 27;
        const MPS_SUPPORT = 1 << 28;
        const SNTF_SUPPORT = 1 << 29;
        const NCQ_SUPPORT = 1 << 30;
        const SUPPORTS_64_ADDRESSES = 1 << 31;
    }
}

bitflags! {
    #[repr(C)]
    pub struct GlobalHBAControl: u32 {
        const HBA_RESET = 1;
        const INT_ENABLE = 1 << 1;
        const MRSM = 1 << 2;
        const AHCI_ENABLE = 1 << 31;
    }
}

const SATA_SIG_ATAPI: u32 = 0xEB140101;
const SATA_SIG_ATA: u32 = 0x00000101;
const SATA_SIG_SEMB: u32 = 0xC33C0101;
const SATA_SIG_PM: u32 = 0x96690101;

const HBA_PORT_DEV_PRESENT: u32 = 0x3;
const HBA_PORT_IPM_ACTIVE: u32 = 0x1;

const HBA_PX_CMD_CR: u32 = 0x8000;
const HBA_PX_CMD_FRE: u32 = 0x0010;
const HBA_PX_CMD_ST: u32 = 0x0001;
const HBA_PX_CMD_FR: u32 = 0x4000;

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
    host_capability: HBACapabilities,
    global_host_control: GlobalHBAControl,
    interrupt_status: u32,
    ports_implemented: u32,
    version: u32,
    ccc_control: u32,
    ccc_ports: u32,
    enclosure_management_location: u32,
    enclosure_management_control: u32,
    host_capabilities_extended: u32,
    bios_handoff_ctrl_sts: u32,
    rsv0: [u8; 116],
    vendor: [u8; 96],
    ports: [MaybeUninit<HBAPort>; 32],
}

impl HBAMemory {
    /// Get a HBA port at `idx`.
    pub fn get_port(&mut self, idx: u32) -> &'static mut HBAPort {
        assert!(idx < 32, "There are only 32 ports!");

        let bit = self.ports_implemented >> idx;

        if bit & 1 == 1 {
            let ptr = self.ports[idx as usize].as_mut_ptr();

            unsafe { &mut *ptr }
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

impl HBAPort {
    fn get_port_type(&self) -> AHCIPortType {
        let ipm = (self.sata_status >> 8) & 0x0F;
        let device_detection = self.sata_status & 0x0F;

        if device_detection != HBA_PORT_DEV_PRESENT {
            return AHCIPortType::None;
        } else if ipm != HBA_PORT_IPM_ACTIVE {
            return AHCIPortType::None;
        }

        match self.signature {
            SATA_SIG_ATAPI => AHCIPortType::SATAPI,
            SATA_SIG_ATA => AHCIPortType::SATA,
            SATA_SIG_PM => AHCIPortType::PM,
            SATA_SIG_SEMB => AHCIPortType::SEMB,

            _ => AHCIPortType::None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct HBACommandHeader {
    command_fis_length: u8,
    atapi: u8,
    write: u8,
    prefetchable: u8,

    reset: u8,
    bist: u8,
    clear_busy: u8,
    rsv0: u8,
    port_multiplier: u8,

    prdt_length: u16,
    prdb_count: u32,
    command_table_base_address: u32,
    command_table_base_address_upper: u32,
    reserved: [u32; 4],
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(128))]
struct HBACommandVector {
    commands: [HBACommandHeader; 32],
}

pub struct Port {
    hba_port: &'static mut HBAPort,
    port_type: AHCIPortType,
    id: u32,
}

impl Port {
    #[inline]
    fn new(hba_port: &'static mut HBAPort, port_type: AHCIPortType, id: u32) -> Self {
        Self {
            hba_port,
            port_type,
            id,
        }
    }

    unsafe fn configure(
        &mut self,
        offset_table: &mut OffsetPageTable,
        frame_allocator: &mut GlobalAllocator,
    ) {
        self.stop_command();

        let mapped_clb = frame_allocator.allocate_frame().unwrap();

        offset_table
            .map_to(
                Page::containing_address(VirtAddr::new(mapped_clb.start_address().as_u64())),
                mapped_clb,
                PageTableFlags::NO_CACHE
                    | PageTableFlags::WRITE_THROUGH
                    | PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE,
                frame_allocator,
            )
            .unwrap()
            .flush();

        self.hba_port.command_list_base = mapped_clb.start_address().as_u64() as u32;
        self.hba_port.command_list_base_upper = (mapped_clb.start_address().as_u64() >> 32) as u32;

        let mapped_fis = frame_allocator.allocate_frame().unwrap();

        offset_table
            .map_to(
                Page::containing_address(VirtAddr::new(mapped_fis.start_address().as_u64())),
                mapped_fis,
                PageTableFlags::NO_CACHE
                    | PageTableFlags::WRITE_THROUGH
                    | PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE,
                frame_allocator,
            )
            .unwrap()
            .flush();

        self.hba_port.fis_base_address = mapped_clb.start_address().as_u64() as u32;
        self.hba_port.fis_base_address = (mapped_clb.start_address().as_u64() >> 32) as u32;

        let hba_command_header = (self.hba_port.command_list_base as u64
            + ((self.hba_port.command_list_base_upper as u64) << 32))
            as *mut HBACommandHeader;

        for i in 0..32 {
            let command_header = &mut *hba_command_header.offset(i);

            // 8 prdt entries per command table.
            command_header.prdt_length = 8;

            let command_table = frame_allocator.allocate_frame().unwrap();

            offset_table
                .map_to(
                    Page::containing_address(VirtAddr::new(command_table.start_address().as_u64())),
                    command_table,
                    PageTableFlags::NO_CACHE
                        | PageTableFlags::WRITE_THROUGH
                        | PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .flush();

            let address = command_table.start_address().as_u64() + ((i as u64) << 8);

            command_header.command_table_base_address = address as u32;
            command_header.command_table_base_address_upper = (address >> 32) as u32;
        }

        self.start_command();
    }

    /// Stop the command engine.
    fn stop_command(&mut self) {
        self.hba_port.cmd_sts &= !HBA_PX_CMD_ST;
        self.hba_port.cmd_sts &= !HBA_PX_CMD_FRE;

        loop {
            if self.hba_port.cmd_sts & HBA_PX_CMD_FR == 1 {
                continue;
            }

            if self.hba_port.cmd_sts & HBA_PX_CMD_CR == 1 {
                continue;
            }

            break;
        }
    }

    /// Start the command engine.
    fn start_command(&mut self) {
        while self.hba_port.cmd_sts & HBA_PX_CMD_CR == 1 {
            unsafe {
                asm!("pause");
            }
        }

        self.hba_port.cmd_sts |= HBA_PX_CMD_FRE;
        self.hba_port.cmd_sts |= HBA_PX_CMD_ST;
    }
}

pub struct AHCI {
    memory: &'static mut HBAMemory,
}

impl AHCI {
    pub unsafe fn new(
        offset_table: &mut OffsetPageTable,
        frame_allocator: &mut GlobalAllocator,
        header: PCIHeader,
    ) -> Self {
        log::info!("Loaded AHCI driver");

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

        let mut this = Self { memory };

        this.probe_ports(offset_table, frame_allocator);

        this
    }

    unsafe fn probe_ports(
        &mut self,
        offset_table: &mut OffsetPageTable,
        frame_allocator: &mut GlobalAllocator,
    ) {
        for i in 0..32 {
            if (self.memory.ports_implemented & (1 << i)) == 1 {
                let hba_port = self.memory.get_port(i);
                let hba_port_type = hba_port.get_port_type();

                if hba_port_type == AHCIPortType::SATA || hba_port_type == AHCIPortType::SATAPI {
                    let mut port = Port::new(hba_port, hba_port_type, i);

                    port.configure(offset_table, frame_allocator);
                }
            }
        }
    }
}
