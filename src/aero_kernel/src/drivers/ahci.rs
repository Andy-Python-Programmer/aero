/*
 * Copyright (C) 2021 The Aero Project Developers.
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
use spin::mutex::SpinMutex;
use spin::Once;

use crate::arch::interrupts;
use crate::mem::paging::*;
use crate::utils::VolatileCell;

use super::pci::*;

static DRIVER: Once<Arc<AhciDriver>> = Once::new();

bitflags::bitflags! {
    struct HbaEnclosureCtrl: u32 {
        const STS_MR =      1 << 0;  // Message Received
        const CTL_TM =      1 << 8;  // Transmit Message
        const CTL_RST =     1 << 9;  // Reset
        const SUPP_LED =    1 << 16; // LED Message Types
        const SUPP_SAFTE =  1 << 17; // SAF-TE Enclosure Management Messages
        const SUPP_SES2 =   1 << 18; // SES-2 Enclosure Management Messages
        const SUPP_SGPIO =  1 << 19; // SGPIO Enclosure Management Messages
        const ATTR_SMB =    1 << 24; // Single Message Buffer
        const ATTR_XMT =    1 << 25; // Transmit Only
        const ATTR_ALHD =   1 << 26; // Activity LED Hardware Driven
        const ATTR_PM =     1 << 27; // Port Multiplier Support
    }
}

bitflags::bitflags! {
    struct HbaCapabilities2: u32 {
        const BOH   = 1 << 0; // BIOS/OS Handoff
        const NVMP  = 1 << 1; // NVMHCI Present
        const APST  = 1 << 2; // Automatic Partial to Slumber Transitions
        const SDS   = 1 << 3; // Supports Device Sleep
        const SADM  = 1 << 4; // Supports Aggressive Device Sleep management
        const DESO  = 1 << 5; // DevSleep Entrance from Slumber Only
    }
}

bitflags::bitflags! {
    struct HbaBohc: u32 {
        const BOS =     1 << 0; // BIOS Owned Semaphore
        const OOS =     1 << 1; // OS Owned Semaphore
        const SOOE =    1 << 2; // SMI on OS Ownership Change Enable
        const OOC =     1 << 3; // OS Ownership Change
        const BB =      1 << 4; // BIOS Busy
    }
}

bitflags::bitflags! {
    struct HbaCapabilities: u32 {
        const SXS           = 1 << 5;  // Supports External SATA
        const EMS           = 1 << 6;  // Enclosure Management Supported
        const CCCS          = 1 << 7;  // Command Completion Coalescing Supported
        const PSC           = 1 << 13; // Partial State Capable
        const SSC           = 1 << 14; // Slumber State Capable
        const PMD           = 1 << 15; // PIO Multiple DRQ Block
        const FBSS          = 1 << 16; // FIS-based Switching Supported
        const SPM           = 1 << 17; // Supports Port Multiplier
        const SAM           = 1 << 18; // Supports AHCI mode only
        const SCLO          = 1 << 24; // Supports Command List Override
        const SAL           = 1 << 25; // Supports Activity LED
        const SALP          = 1 << 26; // Supports Aggressive Link Power Mgmt
        const SSS           = 1 << 27; // Supports Staggered Spin-up
        const SMPS          = 1 << 28; // Supports Mechanical Presence Switch
        const SSNTF         = 1 << 29; // Supports SNotification Register
        const SNCQ          = 1 << 30; // Supports Native Command Queuing
        const S64A          = 1 << 31; // Supports 64-bit Addressing
    }
}

bitflags::bitflags! {
    struct HbaHostCont: u32 {
        const HR =   1 << 0;  // HBA Reset
        const IE =   1 << 1;  // Interrupt Enable
        const MRSM = 1 << 2;  // MSI Revert to Single Message
        const AE =   1 << 31; // AHCI Enable
    }
}

bitflags::bitflags! {
    struct HbaPortIS: u32 {
        const DHRS = 1 << 0; // Device to Host Register FIS Interrupt
        const PSS = 1 << 1; // PIO Setup FIS Interrupt
        const DSS = 1 << 2; // DMA Setup FIS Interrupt
        const SDBS = 1 << 3; // Set Device Bits Interrupt
        const UFS = 1 << 4; // Unknown FIS Interrupt
        const DPS = 1 << 5; // Descriptor Processed
        const PCS = 1 << 6; // Port Connect Change Status
        const DMPS = 1 << 7; // Device Mechanical Presence Status
        const PRCS = 1 << 22; // PhyRdy Change Status
        const IPMS = 1 << 23; // Incorrect Port Multiplier Status
        const OFS = 1 << 24; // Overflow Status
        const INFS = 1 << 26; // Interface Not-fatal Error Status
        const IFS = 1 << 27; // Interface Fatal Error Status
        const HBDS = 1 << 28; // Host Bus Data Error Status
        const HBFS = 1 << 29; // Host Bus Fatal Error Status
        const TFES = 1 << 30; // Task File Error Status
        const CPDS = 1 << 31; // Cold Port Detect Status
    }
}

bitflags::bitflags! {
    struct HbaPortIE: u32 {
        const DHRE = 1 << 0; // Device to Host Register FIS Interrupt
        const PSE = 1 << 1; // PIO Setup FIS Interrupt
        const DSE = 1 << 2; // DMA Setup FIS Interrupt
        const SDBE = 1 << 3; // Set Device Bits Interrupt
        const UFE = 1 << 4; // Unknown FIS Interrupt
        const DPE = 1 << 5; // Descriptor Processed
        const PCE = 1 << 6; // Port Connect Change Status
        const DMPE = 1 << 7; // Device Mechanical Presence Status
        const PRCE = 1 << 22; // PhyRdy Change Status
        const IPME = 1 << 23; // Incorrect Port Multiplier Status
        const OFE= 1 << 24; // Overflow Status
        const INFE = 1 << 26; // Interface Not-fatal Error Status
        const IFE = 1 << 27; // Interface Fatal Error Status
        const HBDE = 1 << 28; // Host Bus Data Error Status
        const HBFE = 1 << 29; // Host Bus Fatal Error Status
        const TFEE = 1 << 30; // Task File Error Status
        const CPDE = 1 << 31; // Cold Port Detect Status
    }
}

bitflags::bitflags! {
    struct HbaPortCmd: u32 {
        const ST = 1 << 0; // Start
        const SUD = 1 << 1; // Spin-Up Device
        const POD = 1 << 2; // Power On Device
        const CLO = 1 << 3; // Command List Override
        const FRE = 1 << 4; // FIS Receive Enable
        const MPSS = 1 << 13; // Mechanical Presence Switch State
        const FR = 1 << 14; // FIS Receive Running
        const CR = 1 << 15; // Command List Running
        const CPS = 1 << 16; // Cold Presence State
        const PMA = 1 << 17; // Port Multiplier Attached
        const HPCP = 1 << 18; // Hot Plug Capable Port
        const MSPC = 1 << 19; // Mechanical Presence Switch Attached to Port
        const CPD = 1 << 20; // Cold Presence Detection
        const ESP = 1 << 21; // External SATA Port
        const FBSCP = 1 << 22; // FIS-based Switching Capable Port
        const APSTE = 1 << 23; // Automatic Partial to Slumber Transition Enabled
        const ATAPI = 1 << 24; // Device is ATAPI
        const DLAE = 1 << 25; // Drive LED on ATAPI Enable
        const ALPE = 1 << 26; // Aggressive Link Power Management Enable
        const ASP = 1 << 27; // Aggressive Slumber / Partial
    }
}

bitflags::bitflags! {
    pub struct HbaCmdHeaderFlags: u16 {
        const A = 1 << 5; // ATAPI
        const W = 1 << 6; // Write
        const P = 1 << 7; // Prefetchable
        const R = 1 << 8; // Reset
        const B = 1 << 9; // Bist
        const C = 1 << 10; // Clear Busy upon R_OK
    }
}

#[repr(C)]
pub struct HbaMemory {
    host_capability: VolatileCell<HbaCapabilities>,
    global_host_control: VolatileCell<HbaHostCont>,
    interrupt_status: VolatileCell<u32>,
    ports_implemented: VolatileCell<u32>,
    version: VolatileCell<u32>,
    ccc_control: VolatileCell<u32>,
    ccc_ports: VolatileCell<u32>,
    enclosure_management_location: VolatileCell<u32>,
    enclosure_management_control: VolatileCell<HbaEnclosureCtrl>,
    host_capabilities_extended: VolatileCell<HbaCapabilities2>,
    bios_handoff_ctrl_sts: VolatileCell<HbaBohc>,
    _reserved: [u8; 0xa0 - 0x2c],
    vendor: [u8; 0x100 - 0xa0],
}

enum HbaPortDd {
    None = 0,
    PresentNotE = 1,
    PresentAndE = 3,
    Offline = 4,
}

enum HbaPortIpm {
    None = 0,
    Active = 1,
    Partial = 2,
    Slumber = 6,
    DevSleep = 8,
}

#[repr(transparent)]
#[derive(Clone, Copy)]
struct HbaSataStatus(u64);

impl HbaSataStatus {
    fn device_detection(&self) -> HbaPortDd {
        match self.0.get_bits(0..=3) {
            0 => HbaPortDd::None,
            1 => HbaPortDd::PresentNotE,
            3 => HbaPortDd::PresentAndE,
            4 => HbaPortDd::Offline,
            v => panic!("Invalid HbaPortSstsRegDet {}", v),
        }
    }

    fn interface_power_management(&self) -> HbaPortIpm {
        match self.0.get_bits(8..=11) {
            0 => HbaPortIpm::None,
            1 => HbaPortIpm::Active,
            2 => HbaPortIpm::Partial,
            6 => HbaPortIpm::Slumber,
            8 => HbaPortIpm::DevSleep,
            v => panic!("Invalid HbaPortSstsRegIpm {}", v),
        }
    }
}

#[repr(C)]
struct HbaPort {
    clb: VolatileCell<PhysAddr>,
    fb: VolatileCell<PhysAddr>,
    is: VolatileCell<HbaPortIS>,
    ie: VolatileCell<HbaPortIE>,
    cmd: VolatileCell<HbaPortCmd>,
    _reserved: u32,
    tfd: VolatileCell<u32>,
    sig: VolatileCell<u32>,
    ssts: VolatileCell<HbaSataStatus>,
    sctl: VolatileCell<u32>,
    serr: VolatileCell<u32>,
    sact: VolatileCell<u32>,
    ci: VolatileCell<u32>,
    sntf: VolatileCell<u32>,
    fbs: VolatileCell<u32>,
    devslp: VolatileCell<u32>,
    _reserved_1: [u32; 10],
    vendor: [u32; 4],
}

#[repr(C)]
struct HbaCmdHeader {
    flags: VolatileCell<HbaCmdHeaderFlags>,
    prdtl: VolatileCell<u16>,
    prdbc: VolatileCell<u32>,
    ctb: VolatileCell<PhysAddr>,
    _reserved: [u32; 4],
}

impl HbaPort {
    pub fn cmd_header_at(&mut self, index: usize) -> &mut HbaCmdHeader {
        // Since the CLB holds the physical address, we make the address mapped
        // before reading it.
        let clb_mapped = unsafe { crate::PHYSICAL_MEMORY_OFFSET + self.clb.get().as_u64() };
        // Get the address of the command header at `index`.
        let clb_addr = clb_mapped + core::mem::size_of::<HbaCmdHeader>() * index;

        // Cast it as [`HbaCmdHeader`] and return a mutable reference to it.
        unsafe { &mut *(clb_addr).as_mut_ptr::<HbaCmdHeader>() }
    }

    /// This function is responsible for allocating space for command lists,
    /// tables, etc.. for a given this instance of HBA port.
    pub fn start(&mut self) {
        self.stop_cmd(); // Stop the command engine before starting the port

        // Allocate area for for the command list.
        let frame = unsafe { FRAME_ALLOCATOR.allocate_frame() }
            .expect("Failed to allocate space for the command list");

        self.clb.set(frame.start_address());

        // Allocate area for FISs.
        let frame = unsafe { FRAME_ALLOCATOR.allocate_frame() }
            .expect("Failed to allocate space for the FISs");

        // Set the address that received FISes will be copied to.
        self.fb.set(frame.start_address());

        for i in 0..32 {
            let frame = unsafe { FRAME_ALLOCATOR.allocate_frame() }
                .expect("Here is a nickel kid, go and buy your self a real computer");

            let command_header = self.cmd_header_at(i);

            // 8 prdt entries per command table
            // 256 bytes per command table, 64 + 16 + 48 + 16 * 8
            command_header.prdtl.set(8);
            command_header.prdbc.set(0);
            command_header.ctb.set(frame.start_address());
        }

        self.start_cmd(); // Start the command engine...
    }

    pub fn start_cmd(&mut self) {
        while self.cmd.get().contains(HbaPortCmd::CR) {
            interrupts::pause();
        }

        let value = self.cmd.get() | (HbaPortCmd::FRE | HbaPortCmd::ST);
        self.cmd.set(value);
    }

    pub fn stop_cmd(&mut self) {
        let mut cmd = self.cmd.get();
        cmd.remove(HbaPortCmd::FRE | HbaPortCmd::ST);

        self.cmd.set(cmd);

        while self.cmd.get().intersects(HbaPortCmd::FR | HbaPortCmd::CR) {
            interrupts::pause();
        }
    }

    pub fn probe(&mut self, port: usize) -> bool {
        let status = self.ssts.get();

        let ipm = status.interface_power_management();
        let dd = status.device_detection();

        // Check if the port is active and is present. If thats the case
        // we can start the AHCI port.
        if let (HbaPortDd::PresentAndE, HbaPortIpm::Active) = (dd, ipm) {
            log::trace!("Enabling AHCI port {}", port);

            self.start();
            true
        } else {
            // Else we can't enable the port.
            false
        }
    }
}

impl HbaMemory {
    fn port_mut(&mut self, port: usize) -> &mut HbaPort {
        unsafe { &mut *((self as *mut Self).offset(1) as *mut HbaPort).offset(port as isize) }
    }
}

struct AhciPortProtected {
    address: VirtAddr,
}

impl AhciPortProtected {
    fn hba_port(&mut self) -> &mut HbaPort {
        unsafe { &mut *(self.address.as_mut_ptr::<HbaPort>()) }
    }

    fn run_request(&mut self) {
        let _port = self.hba_port();
    }
}

struct AhciPort {
    inner: SpinMutex<AhciPortProtected>,
}

impl AhciPort {
    #[inline]
    fn new(address: VirtAddr) -> Self {
        Self {
            inner: SpinMutex::new(AhciPortProtected { address }),
        }
    }
}

struct AhciProtected {
    ports: [Option<Arc<AhciPort>>; 32],
    hba: VirtAddr,
}

impl AhciProtected {
    #[inline]
    fn hba_mem(&self) -> &mut HbaMemory {
        unsafe { &mut *(self.hba.as_u64() as *mut HbaMemory) }
    }

    fn start_hba(&mut self) {
        let mut hba = self.hba_mem();
        let current_flags = hba.global_host_control.get();

        hba.global_host_control.set(current_flags | HbaHostCont::IE); // Enable Interrupts

        let pi = hba.ports_implemented.get();

        for i in 0..32 {
            if pi.get_bit(i) {
                let port = hba.port_mut(i);

                if port.probe(i) {
                    // Get the address of the HBA port.
                    let address = unsafe { VirtAddr::new_unsafe(port as *const _ as _) };

                    drop(port); // Drop the reference to the port.
                    drop(hba); // Drop the reference to the HBA.

                    let port = Arc::new(AhciPort::new(address));

                    // Add the port to the ports array.
                    self.ports[i] = Some(port);

                    // Workaround to get access to the HBA and still satify the
                    // borrow checker.
                    hba = self.hba_mem();
                }
            }
        }
    }

    /// This function is responsible for initializing and starting the AHCI driver.
    fn start_driver(&mut self, header: &PciHeader) -> Result<(), MapToError<Size4KiB>> {
        let abar = unsafe { header.get_bar(5).expect("Failed to get ABAR") };

        let (abar_address, _) = match abar {
            Bar::Memory32 { address, size, .. } => (address as u64, size as u64),
            Bar::Memory64 { address, size, .. } => (address, size),
            Bar::IO { .. } => panic!("ABAR is in port space o_O"),
        };

        self.hba = unsafe { crate::PHYSICAL_MEMORY_OFFSET + abar_address }; // Update the HBA address.

        self.start_hba();

        Ok(())
    }
}

/// Structure representing the ACHI driver.
struct AhciDriver {
    inner: SpinMutex<AhciProtected>,
}

impl PciDeviceHandle for AhciDriver {
    fn handles(&self, vendor_id: Vendor, device_id: DeviceType) -> bool {
        match (vendor_id, device_id) {
            (Vendor::Intel, DeviceType::SataController) => true,

            _ => false,
        }
    }

    fn start(&self, header: &PciHeader, _offset_table: &mut OffsetPageTable) {
        log::info!("Starting AHCI driver...");

        // Disable interrupts as we do not want to be interrupted durning
        // the initialization of the AHCI driver.
        unsafe {
            interrupts::disable_interrupts();
        }

        get_ahci().inner.lock().start_driver(header).unwrap(); // Start and initialize the AHCI controller.

        // Temporary testing...
        if let Some(port) = get_ahci().inner.lock().ports[0].clone() {
            port.inner.lock().run_request();
        }

        // Now the AHCI driver is initialized, we can enable interrupts.
        unsafe {
            interrupts::enable_interrupts();
        }
    }
}

/// Returns a reference-counting pointer to the AHCI driver.
fn get_ahci() -> &'static Arc<AhciDriver> {
    DRIVER
        .get()
        .expect("Attempted to get the AHCI driver before it was initialized")
}

/// This function is responsible for initializing and running the AHCI driver.
pub fn ahci_init() {
    // Initialize the AHCI driver instance.
    DRIVER.call_once(|| {
        const EMPTY: Option<Arc<AhciPort>> = None; // To satisfy the Copy trait bound when the AHCI creating data.

        Arc::new(AhciDriver {
            inner: SpinMutex::new(AhciProtected {
                ports: [EMPTY; 32],    // Initialize the AHCI ports to an empty slice.
                hba: VirtAddr::zero(), // Initialize the AHCI HBA address to zero.
            }),
        })
    });

    // Now register the AHCI driver with the PCI subsystem.
    register_device_driver(get_ahci().clone());
}

crate::module_init!(ahci_init);
