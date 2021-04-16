use core::mem::{self, MaybeUninit};

use crate::arch::memory::paging::{memory_map_device, GlobalAllocator};

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

const ATA_DEV_BUSY: u32 = 0x80;
const ATA_DEV_DRQ: u32 = 0x08;
const ATA_CMD_READ_DMA_EX: u32 = 0x25;

const FIS_TYPE_REG_H2D: u32 = 0x27;
const HBA_PXIS_TFES: u32 = 1 << 30;

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
struct HBACommandHeader {
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
#[repr(C, packed)]
struct FISRegisterH2D {
    fis_type: u32,

    port_multiplier: u32,
    rsv0: u32,
    command_control: u32,

    command: u32,
    feature_low: u32,

    lba0: u32,
    lba1: u32,
    lba2: u32,
    device_register: u32,

    lba3: u32,
    lba4: u32,
    lba5: u32,
    feature_high: u32,

    count_low: u32,
    count_high: u32,
    iso_command_completion: u32,
    control: u32,

    reserved: [u32; 4],
}

#[repr(C, packed)]
struct HBAPRDTEntry {
    data_base_address: u32,
    data_base_address_upper: u32,
    reserved: u32,

    byte_count: u32,
    reserved_2: u32,
    interrupt_on_completion: u32,
}

#[repr(C, align(128))]
struct HBACommandTable {
    command_fis: [u8; 64],
    atapi_command: [u8; 16],
    reserved: [u8; 48],

    hba_prdt_entry: [HBAPRDTEntry; 8],
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

    pub fn read(&mut self, sector: u64, sector_count: u32, buffer: &mut [u16]) -> bool {
        let mut spin = 0; // Spin lock timeout counter.

        while (self.hba_port.task_file_data & (ATA_DEV_BUSY | ATA_DEV_DRQ)) == 1 && spin < 1000000 {
            spin += 1;
        }

        if spin == 1000000 {
            return false;
        }

        let sector_low = sector as u32;
        let sector_hi = (sector >> 32) as u32;

        // Clear the pending interrupt bits.
        self.hba_port.interrupt_status = !0;

        let command_header =
            unsafe { &mut *(self.hba_port.command_list_base as *mut HBACommandHeader) };

        command_header.command_fis_length =
            (mem::size_of::<FISRegisterH2D>() / mem::size_of::<u32>()) as u8;
        command_header.write = 0; // Set write to 0x00 as we are doing a read command.
        command_header.prdt_length = 1;

        let command_table =
            (command_header.command_table_base_address as u64) as *mut HBACommandTable;

        let command_table = unsafe { &mut *command_table };

        command_table.hba_prdt_entry[0].data_base_address = buffer.as_mut_ptr() as u32;
        command_table.hba_prdt_entry[0].data_base_address_upper =
            ((buffer.as_mut_ptr() as u64) >> 32) as u32;
        command_table.hba_prdt_entry[0].byte_count = (sector_count << 9) - 1;
        command_table.hba_prdt_entry[0].interrupt_on_completion = 1; // TODO: Function on completion.

        let command_fis_address = unsafe { *(&command_table.command_fis as *const u8) as usize };
        let command_fis = unsafe { &mut *(command_fis_address as *mut FISRegisterH2D) };

        command_fis.fis_type = FIS_TYPE_REG_H2D;
        command_fis.command_control = 1;
        command_fis.command = ATA_CMD_READ_DMA_EX;

        command_fis.lba0 = sector_low;
        command_fis.lba1 = sector_low >> 8;
        command_fis.lba2 = sector_low >> 16;

        command_fis.lba3 = sector_hi;
        command_fis.lba4 = sector_hi >> 8;
        command_fis.lba5 = sector_hi >> 16;

        command_fis.device_register = 1 << 6; // LBA mode

        command_fis.count_low = sector_count & 0xFF;
        command_fis.count_high = (sector_count >> 8) & 0xFF;

        self.hba_port.command_issue = 0x1;

        loop {
            if self.hba_port.command_list_base == 0 {
                break;
            } else if self.hba_port.interrupt_status & HBA_PXIS_TFES == 1 {
                return false;
            }
        }

        return true;
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
                    let mut buffer = [0u16; 256];

                    let mut port = Port::new(hba_port, hba_port_type, i);

                    port.configure(offset_table, frame_allocator);
                    port.read(0, 4, &mut buffer);
                }
            }
        }
    }
}
