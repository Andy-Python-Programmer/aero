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

mod command;
mod dma;
mod queue;

use core::mem::MaybeUninit;

use command::*;
use dma::*;
use queue::*;

use alloc::sync::Arc;
use alloc::vec::Vec;

use bit_field::BitField;

use crate::arch::interrupts::{self, InterruptStack};
use crate::drivers::pci::*;
use crate::fs::block::{install_block_device, BlockDevice, BlockDeviceInterface};
use crate::mem::paging::*;

use crate::utils::sync::Mutex;
use crate::utils::{CeilDiv, VolatileCell};

#[derive(Copy, Clone, Debug)]
enum Error {
    UnknownBar,
    NotSupported,
    ControllerFatal,
    NotMsixCapable,
}

#[repr(transparent)]
struct Version(VolatileCell<u32>);

impl Version {
    fn major(&self) -> u16 {
        self.0.get().get_bits(16..32) as u16
    }

    fn minor(&self) -> u8 {
        self.0.get().get_bits(8..16) as u8
    }

    fn tertiary(&self) -> u8 {
        self.0.get().get_bits(0..8) as u8
    }
}

bitflags::bitflags! {
    struct CommandSetsSupported: u8 {
        /// Controller supports the NVM command set.
        const NVM = 1 << 0;
        /// Controller supports one or more I/O Command Sets and supports
        /// the Identify I/O Command Set data structure.
        const IO = 1 << 6;
        const ADMIN = 1 << 7;
    }
}

#[repr(transparent)]
struct Capability(VolatileCell<u64>);

impl Capability {
    /// Returns maximum individual queue size that the controller
    /// supports.
    fn max_queue_entries(&self) -> u16 {
        self.0.get().get_bits(0..16) as u16
    }

    /// Returns the stride between doorbell properties.
    fn get_doorbell_stride(&self) -> u64 {
        self.0.get().get_bits(32..36)
    }

    /// Returns the command sets that are supported by the
    /// controller.
    fn get_css(&self) -> CommandSetsSupported {
        CommandSetsSupported::from_bits_truncate(self.0.get().get_bits(37..45) as u8)
    }

    /// Returns the the minimum host memory page size that the
    /// controller supports.
    fn mpsmin(&self) -> u64 {
        self.0.get().get_bits(48..52)
    }
}

#[repr(u32)]
enum CommandSet {
    NVM = 0b000,
}

const_assert_eq!(core::mem::size_of::<CommandSet>(), 4);

#[repr(u32)]
enum ArbitrationMechanism {
    RoundRobin = 0b000,
}

const_assert_eq!(core::mem::size_of::<ArbitrationMechanism>(), 4);

#[repr(transparent)]
struct ControllerConfig(VolatileCell<u32>);

impl ControllerConfig {
    /// Sets the I/O submission queue size to `size`.
    fn set_iosqes(&mut self, size: u32) {
        let mut cfg = self.0.get();
        cfg.set_bits(16..20, size);
        self.0.set(cfg);
    }

    /// Sets the I/O completion queue size to `size`.
    fn set_iocqes(&mut self, size: u32) {
        let mut cfg = self.0.get();
        cfg.set_bits(20..24, size);
        self.0.set(cfg);
    }

    /// Sets the arbitration mechanism to be used.
    fn set_ams(&mut self, ams: ArbitrationMechanism) {
        let mut cfg = self.0.get();
        cfg.set_bits(11..14, ams as u32);
        self.0.set(cfg);
    }

    /// Sets the command set to be used.
    fn set_css(&mut self, command_set: CommandSet) {
        // XXX: This field shall only be changed when the controller is disabled.
        assert_eq!(self.is_enabled(), false);

        let mut cfg = self.0.get();
        cfg.set_bits(4..7, command_set as u32);
        self.0.set(cfg);
    }

    /// Returns whether the controller is enable. See the documentation for
    /// [`Self::set_enable`] for more information.
    fn is_enabled(&self) -> bool {
        self.0.get().get_bit(0)
    }

    /// Sets the enable bit if `enable` is true, otherwise clears the
    /// enable bit.
    ///
    /// When the enable bit is set, the shall process commands. On the other
    /// hand, when the bit is cleared, then the controller shall not process
    /// commands nor post completion queue entries to completion queues.
    fn set_enable(&mut self, enable: bool) {
        let mut cfg = self.0.get();
        cfg.set_bit(0, enable);
        self.0.set(cfg);
    }
}

#[repr(transparent)]
struct ControllerStatus(VolatileCell<u32>);

impl ControllerStatus {
    /// Returns whether the controller is in a ready state.
    fn is_ready(&self) -> bool {
        self.0.get().get_bit(0)
    }

    /// Returns whether the `CFS` (Controller Fatal Status) bit is set.
    fn get_cfs(&self) -> bool {
        self.0.get().get_bit(1)
    }
}

#[repr(C)]
struct Registers {
    capability: Capability,
    version: Version,
    intms: u32,
    intmc: u32,
    cc: ControllerConfig,
    rsvd1: u32,
    controller_status: ControllerStatus,
    rsvd2: u32,
    aqa: VolatileCell<u32>,
    asq: VolatileCell<u64>,
    acq: VolatileCell<u64>,
}

impl Registers {
    fn set_enable(&mut self, enable: bool) -> Result<(), Error> {
        log::trace!("nvme: resetting the controller to enabled={} state", enable);
        // XXX: The transition from an enabled to disabled state resets the controller.
        self.cc.set_enable(enable);

        while self.controller_status.is_ready() != enable {
            core::hint::spin_loop();
        }

        if self.controller_status.get_cfs() {
            Err(Error::ControllerFatal)
        } else {
            Ok(())
        }
    }
}

struct Namespace<'a> {
    nsid: u32,
    blocks: usize,
    block_size: usize,
    size: usize,
    max_prps: usize,
    prps: Mutex<Dma<[MaybeUninit<u64>]>>,
    controller: Arc<Controller<'a>>,
}

impl<'a> Namespace<'a> {
    fn read(&self, sector: usize, start: PhysAddr, size_bytes: usize) {
        assert!(size_bytes != 0);

        let blocks = size_bytes.ceil_div(self.block_size);
        let mut read_cmd = ReadWriteCommand::default();

        read_cmd.opcode = CommandOpcode::Read as u8;
        read_cmd.nsid = self.nsid;
        read_cmd.start_lba = sector as u64;
        read_cmd.length = (blocks - 1) as u16;

        if size_bytes > Size4KiB::SIZE as usize {
            // The data cannot fit in 8KiB frames, so we need to use
            // a PRP list.
            let prp_num = ((blocks - 1) * self.block_size) / Size4KiB::SIZE as usize;
            assert!(prp_num < self.max_prps);

            let mut prps = self.prps.lock();

            for i in 0..prp_num {
                prps[i].write((start.as_u64() + Size4KiB::SIZE) + (Size4KiB::SIZE * i as u64));
            }

            read_cmd.data_ptr.prp1 = start.as_u64();
            read_cmd.data_ptr.prp2 = prps.addr().as_u64();
        } else {
            read_cmd.data_ptr.prp1 = start.as_u64();
        }

        self.controller.io_queue.lock_irq().submit_command(read_cmd);
    }
}

struct Controller<'a> {
    identity: Dma<IdentifyController>,
    namespaces: Mutex<Vec<Namespace<'a>>>,

    admin: Mutex<QueuePair<'a>>,
    io_queue: Mutex<QueuePair<'a>>,
}

impl<'a> Controller<'a> {
    fn new(header: &PciHeader) -> Result<Arc<Self>, Error> {
        log::trace!("nvme: setting up NVMe controller");

        header.enable_bus_mastering();
        header.enable_mmio();

        let bar0 = header.get_bar(0).ok_or(Error::UnknownBar)?;

        // All NVMe registers are accessible via BAR0.
        let registers_addr = match bar0 {
            Bar::Memory64 { address, .. } => PhysAddr::new(address),
            _ => return Err(Error::UnknownBar),
        };

        let registers = registers_addr
            .as_hhdm_virt()
            .read_mut::<Registers>()
            .unwrap();

        log::trace!(
            "nvme: version (major={}, minor={}, tertiary={})",
            registers.version.major(),
            registers.version.minor(),
            registers.version.tertiary()
        );

        let mut msix = header.msix().ok_or(Error::NotMsixCapable)?;

        let vector = interrupts::allocate_vector();
        interrupts::register_handler(vector, irq_handler);

        msix.set(vector);

        // Check the capabilities register for support of the NVM command set.
        let css = registers.capability.get_css();

        if !css.contains(CommandSetsSupported::NVM) {
            log::error!("nvme: controller does not support the NVM command set (css={css:?})");
            return Err(Error::NotSupported);
        }

        // Reset the controller.
        registers.set_enable(false)?;

        let queue_size = registers.capability.max_queue_entries() as usize;

        let mut admin = QueuePair::new(&registers, queue_size)?;

        registers
            .aqa
            // 28..32 = Reserved
            // 16..28 = Admin Completion Queue Size (ACQS)
            // 0..12  = Admin Submission Queue Size (ASQS)
            .set(((queue_size - 1) << 16 | (queue_size - 1)) as u32);

        registers.asq.set(admin.submission_addr().as_u64());
        registers.acq.set(admin.completion_addr().as_u64());

        // Set the controller configuration and admin queue base addresses.
        registers.cc.set_css(CommandSet::NVM);
        registers.cc.set_ams(ArbitrationMechanism::RoundRobin);
        registers.cc.set_iosqes(6); // 64 bytes
        registers.cc.set_iocqes(4); // 16 bytes

        registers.set_enable(true)?;

        let identity = Dma::<IdentifyController>::new();
        let mut identify_command = IdentifyCommand::default();

        identify_command.opcode = AdminOpcode::Identify as u8;
        identify_command.cns = IdentifyCns::Controller as u8;
        identify_command.data_ptr.prp1 = identity.addr().as_u64();

        admin.submit_command(identify_command);

        log::trace!(
            "nvme: identifed controller (vendor={}, subsystem_vendor={})",
            identity.vid,
            identity.ssvid
        );

        // Create and initialize the I/O queues.
        let io_queue = QueuePair::new(&registers, queue_size)?;

        let mut io_cq_cmd = CreateCQCommand::default();

        io_cq_cmd.opcode = AdminOpcode::CreateCq as u8;
        io_cq_cmd.prp1 = io_queue.completion_addr().as_u64();
        io_cq_cmd.cqid = io_queue.id();
        io_cq_cmd.q_size = (io_queue.len() - 1) as u16;
        io_cq_cmd.irq_vector = 0;
        io_cq_cmd.cq_flags = CommandFlags::QUEUE_PHYS_CONTIG.bits();

        admin.submit_command(io_cq_cmd);

        let mut io_sq_cmd = CreateSQCommand::default();

        io_sq_cmd.opcode = AdminOpcode::CreateSq as u8;
        io_sq_cmd.prp1 = io_queue.submission_addr().as_u64();
        io_sq_cmd.cqid = io_queue.id();
        io_sq_cmd.sqid = io_queue.id();
        io_sq_cmd.q_size = (io_queue.len() - 1) as u16;
        io_sq_cmd.sq_flags = CommandFlags::QUEUE_PHYS_CONTIG.bits();

        admin.submit_command(io_sq_cmd);

        let shift = 12 + registers.capability.mpsmin() as usize;
        let max_transfer_shift = if identity.mdts != 0 {
            shift + identity.mdts as usize
        } else {
            20
        };

        let this = Arc::new(Self {
            identity,
            namespaces: Mutex::new(alloc::vec![]),

            admin: Mutex::new(admin),
            io_queue: Mutex::new(io_queue),
        });

        // Discover and initialize the namespaces.
        let nsids = {
            let nsid_list = Dma::<u32>::new_uninit_slice(this.identity.nn as usize);
            let mut nsid_command = IdentifyCommand::default();

            nsid_command.opcode = AdminOpcode::Identify as u8;
            nsid_command.cns = IdentifyCns::ActivateList as u8;
            nsid_command.data_ptr.prp1 = nsid_list.addr().as_u64();

            this.admin.lock().submit_command(nsid_command);

            // SAFETY: The list is initialized above.
            unsafe { nsid_list.assume_init() }
        };

        let mut namespaces = alloc::vec![];

        for &nsid in nsids.iter() {
            // Unused entries are zero-filled.
            if nsid == 0 {
                continue;
            }

            let identity = Dma::<IdentifyNamespace>::new();
            let mut identify_command = IdentifyCommand::default();

            identify_command.opcode = AdminOpcode::Identify as u8;
            identify_command.cns = IdentifyCns::Namespace as u8;
            identify_command.nsid = nsid;
            identify_command.data_ptr.prp1 = identity.addr().as_u64();

            this.admin.lock().submit_command(identify_command);

            let blocks = identity.nsze as usize;
            let block_size = 1 << identity.lbaf[(identity.flbas & 0b11111) as usize].ds;

            // The maximum transfer size is in units of 2^(min page size)
            let lba_shift = identity.lbaf[(identity.flbas & 0xf) as usize].ds;
            let max_lbas = 1 << (max_transfer_shift - lba_shift as usize);
            let max_prps = (max_lbas * (1 << lba_shift)) / Size4KiB::SIZE as usize;

            let namespace = Namespace {
                controller: this.clone(),
                nsid,
                blocks,
                block_size,
                size: blocks * block_size,
                max_prps,
                prps: Mutex::new(Dma::new_uninit_slice(max_prps)),
            };

            log::trace!(
                "nvme: identified namespace (blocks={}, block_size={}, size={})",
                namespace.blocks,
                namespace.block_size,
                namespace.size
            );

            namespaces.push(namespace);
        }

        *this.namespaces.lock() = namespaces;

        log::trace!("nvme: successfully initialized NVMe controller");
        Ok(this)
    }
}

impl<'a> BlockDeviceInterface for Controller<'a> {
    fn read_dma(&self, sector: usize, start: PhysAddr, size: usize) -> Option<usize> {
        self.namespaces.lock()[0].read(sector, start, size);
        Some(size)
    }

    fn read_block(&self, sector: usize, dest: &mut [MaybeUninit<u8>]) -> Option<usize> {
        let buffer = Dma::<u8>::new_uninit_slice(dest.len());
        self.namespaces.lock()[0].read(sector, buffer.addr(), dest.len());

        // SAFETY: The buffer is initialized above.
        dest.copy_from_slice(&buffer);
        Some(dest.len())
    }

    fn block_size(&self) -> usize {
        self.namespaces.lock()[0].block_size
    }

    fn write_block(&self, _sector: usize, _buf: &[u8]) -> Option<usize> {
        unimplemented!()
    }
}

// PCI device handler for NVMe controllers.
struct Handler<'admin> {
    controllers: Mutex<Vec<Arc<Controller<'admin>>>>,
}

impl<'admin> Handler<'admin> {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            controllers: Mutex::new(Vec::new()),
        })
    }
}

impl PciDeviceHandle for Handler<'static> {
    fn handles(&self, _vendor_id: Vendor, device_id: DeviceType) -> bool {
        device_id == DeviceType::NvmeController
    }

    fn start(&self, header: &PciHeader, _offset_table: &mut OffsetPageTable) {
        let controller = Controller::new(header).expect("nvme: failed to init the controller");
        let controller_id = self.controllers.lock().len();

        // Register the block devices; NVME storage namespaces.
        let devices = controller
            .namespaces
            .lock()
            .iter()
            .map(|namespace| alloc::format!("nvme{}n{}", controller_id, namespace.nsid))
            .collect::<Vec<_>>();

        for device_name in devices {
            let device = BlockDevice::new(device_name, controller.clone());
            install_block_device(device).expect("nvme: failed to install the block device")
        }

        self.controllers.lock().push(controller);
    }
}

fn irq_handler(_stack: &mut InterruptStack) {
    unimplemented!("nvme: interrupt handler!")
}

fn nvme_init() {
    // Register the NVMe device handler.
    register_device_driver(Handler::new())
}

crate::module_init!(nvme_init);
