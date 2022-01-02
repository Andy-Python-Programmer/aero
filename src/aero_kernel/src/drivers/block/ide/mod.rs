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

mod channel;
mod registers;

use channel::*;

use alloc::sync::Arc;
use spin::Once;

use crate::drivers::pci::*;

use crate::fs::block;
use crate::fs::block::{BlockDevice, BlockDeviceInterface};

use crate::mem::paging::OffsetPageTable;
use crate::utils::sync::Mutex;
use crate::utils::CeilDiv;

use super::ahci::DmaRequest;

static DRIVER: Once<Arc<Ide>> = Once::new();

pub struct IdeDrive {
    slave: bool,
    channel: Arc<IdeChannel>,
}

impl IdeDrive {
    pub fn new(slave: bool, channel: Arc<IdeChannel>) -> Arc<IdeDrive> {
        Arc::new(IdeDrive { slave, channel })
    }
}

impl BlockDeviceInterface for IdeDrive {
    fn read(&self, sector: usize, dest: &mut [u8]) -> Option<usize> {
        let count = dest.len().ceil_div(512);
        let request = Arc::new(DmaRequest::new(sector, count));

        let res = self.channel.run_request(request.clone(), self.slave);

        if res.is_some() {
            request.copy_into(dest);
        }

        res
    }

    fn write(&self, _sector: usize, _buf: &[u8]) -> Option<usize> {
        unimplemented!()
    }
}

pub struct IdeDevice {
    ide_devs: [Option<Arc<IdeDrive>>; 4],
    channels: [Option<Arc<IdeChannel>>; 2],
}

impl IdeDevice {
    pub fn new() -> IdeDevice {
        const EMPTY_DRIVE: Option<Arc<IdeDrive>> = None;
        const EMPTY_CHANNELS: Option<Arc<IdeChannel>> = None;

        IdeDevice {
            ide_devs: [EMPTY_DRIVE; 4],
            channels: [EMPTY_CHANNELS; 2],
        }
    }
}

impl IdeDevice {
    fn launch(&mut self, header: &PciHeader) {
        log::trace!("ide: starting ide");

        let program_interface = header.program_interface();

        if program_interface.contains(ProgramInterface::DMA_CAPABLE) {
            log::warn!("ide: dma not supported");
            return;
        }

        if header.get_header_type() != 0 {
            log::debug!("ide: header type != 0");
            return;
        }

        let bmid_1 = header.base_address4() & 0xFFFF_FFFC;
        let bmid_2 = bmid_1 + 8;

        let (io1, io2) = {
            (
                if header.base_address0() != 0 {
                    header.base_address0() & 0xFFFF_FFFC
                } else {
                    0x1F0
                },
                if header.base_address1() != 0 {
                    header.base_address1() & 0xFFFF_FFFC
                } else {
                    0x3F6
                },
            )
        };

        let (io3, io4) = {
            (
                if header.base_address2() != 0 {
                    header.base_address2() & 0xFFFF_FFFC
                } else {
                    0x170
                },
                if header.base_address3() != 0 {
                    header.base_address3() & 0xFFFF_FFFC
                } else {
                    0x376
                },
            )
        };

        let c1 = IdeChannel::new(io1 as u16, io2 as u16, bmid_1 as u16, 14);
        let c2 = IdeChannel::new(io3 as u16, io4 as u16, bmid_2 as u16, 15);

        let mut idx = 0;
        for (ci, c) in [c1, c2].iter().enumerate() {
            for &s in [false, true].iter() {
                if c.detect(s) {
                    self.ide_devs[idx] = Some(IdeDrive::new(s, c.clone()));
                    idx += 1;

                    if self.channels[ci].is_none() {
                        self.channels[ci] = Some(c.clone());
                    }
                }
            }
        }

        if idx > 0 {
            header.enable_bus_mastering();

            for channel in self
                .channels
                .iter_mut()
                .filter(|a| a.is_some())
                .map(|a| a.as_mut().unwrap())
            {
                channel.init();
            }

            for (i, drive) in self
                .ide_devs
                .iter()
                .filter(|e| e.is_some())
                .map(|e| e.as_ref().unwrap())
                .enumerate()
            {
                let name = alloc::format!("blck{}", i);
                let block_device = BlockDevice::new(name, drive.clone());

                block::install_block_device(block_device).unwrap();
            }
        }
    }
}

struct Ide {
    device: Mutex<IdeDevice>,
}

impl PciDeviceHandle for Ide {
    fn handles(&self, vendor_id: Vendor, device_id: DeviceType) -> bool {
        match (vendor_id, device_id) {
            (Vendor::Intel, DeviceType::IdeController) => true,

            _ => false,
        }
    }

    fn start(&self, header: &PciHeader, _offset_table: &mut OffsetPageTable) {
        self.device.lock_irq().launch(header);
    }
}

fn get_device() -> &'static Arc<Ide> {
    DRIVER
        .get()
        .expect("ide: attempted to get the ide driver instance before it was initialized")
}

fn init() {
    DRIVER.call_once(|| {
        Arc::new(Ide {
            device: Mutex::new(IdeDevice::new()),
        })
    });

    register_device_driver(get_device().clone());
}

crate::module_init!(init);
