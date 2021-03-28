use super::pci::PCIHeader;
use crate::log;

pub struct AHCI {
    header: PCIHeader,
}

impl AHCI {
    pub unsafe fn new(header: PCIHeader) -> Self {
        log::info("Loaded AHCI driver");

        Self { header }
    }
}
