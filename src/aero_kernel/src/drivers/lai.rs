use alloc::sync::Arc;

use crate::acpi::fadt;
use crate::acpi::get_acpi_table;

use crate::mem::paging::PhysAddr;

use crate::userland::scheduler;
use crate::utils::io;

use super::pci::PciHeader;

struct LaiHost;

impl lai::Host for LaiHost {
    fn scan(&self, signature: &str, index: usize) -> *const u8 {
        assert!(index == 0);

        if signature == "DSDT" {
            // The DSDT table is put inside the FADT table, instead of listing it in
            // another ACPI table. So, we need to extract the DSDT table from the FADT
            // table.
            get_acpi_table().lookup_entry(fadt::SIGNATURE).map(|fadt| {
                let fadt: &'static fadt::Fadt = unsafe { fadt.as_ref() };
                let addr = PhysAddr::new(fadt.dsdt as u64).as_hhdm_virt();
                addr.as_ptr::<u8>()
            })
        } else {
            get_acpi_table()
                .lookup_entry(signature)
                .map(|table| table as *const _ as *const u8)
        }
        .unwrap_or(core::ptr::null())
    }

    fn sleep(&self, ms: u64) {
        scheduler::get_scheduler()
            .inner
            .sleep(Some(ms as usize / 1000))
            .expect("lai: unexpected signal during sleep")
    }

    // Port I/O functions:
    #[inline]
    fn outb(&self, port: u16, value: u8) {
        unsafe { io::outb(port, value) }
    }

    #[inline]
    fn outw(&self, port: u16, value: u16) {
        unsafe { io::outw(port, value) }
    }

    #[inline]
    fn outd(&self, port: u16, value: u32) {
        unsafe { io::outl(port, value) }
    }

    #[inline]
    fn inb(&self, port: u16) -> u8 {
        unsafe { io::inb(port) }
    }

    #[inline]
    fn inw(&self, port: u16) -> u16 {
        unsafe { io::inw(port) }
    }

    #[inline]
    fn ind(&self, port: u16) -> u32 {
        unsafe { io::inl(port) }
    }

    // PCI read functions:
    //
    // todo: do not ignore the segment once we use MCFG.
    fn pci_readb(&self, _seg: u16, bus: u8, slot: u8, fun: u8, offset: u16) -> u8 {
        let header = PciHeader::new(bus, slot, fun);
        unsafe { header.read::<u8>(offset as u32) as u8 }
    }

    fn pci_readw(&self, _seg: u16, bus: u8, slot: u8, fun: u8, offset: u16) -> u16 {
        let header = PciHeader::new(bus, slot, fun);
        unsafe { header.read::<u16>(offset as u32) as u16 }
    }

    fn pci_readd(&self, _seg: u16, bus: u8, slot: u8, fun: u8, offset: u16) -> u32 {
        let header = PciHeader::new(bus, slot, fun);
        unsafe { header.read::<u32>(offset as u32) }
    }

    // Memory functions:
    #[inline]
    fn map(&self, address: usize, _count: usize) -> *mut u8 {
        PhysAddr::new(address as u64)
            .as_hhdm_virt()
            .as_mut_ptr::<u8>()
    }
}

pub fn init_lai() {
    let lai_host = Arc::new(LaiHost);
    lai::init(lai_host);

    lai::set_acpi_revision(get_acpi_table().revision() as _);
    lai::create_namespace();

    lai::enable_acpi(1);
}

crate::module_init!(init_lai);
