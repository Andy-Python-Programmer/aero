//! The ACPI (Advanced Configuration and Power Interface) tables help to gather the
//! CPU, interrupt, and timer informations.
//!
//! **Notes**: <https://wiki.osdev.org/ACPI>

use core::mem;

use x86_64::{structures::paging::*, PhysAddr, VirtAddr};

use crate::mem::paging::FRAME_ALLOCATOR;

use self::{fadt::Fadt, hpet::Hpet, madt::Madt, mcfg::Mcfg, rsdp::Rsdp, sdt::Sdt};

pub mod fadt;
pub mod hpet;
pub mod madt;
pub mod mcfg;
pub mod rsdp;
pub mod sdt;

enum AcpiHeader {
    Rsdt(&'static Sdt),
    Xsdt(&'static Sdt),
}

impl AcpiHeader {
    fn as_sdt(&self) -> &'static Sdt {
        match self {
            AcpiHeader::Rsdt(rsdt) => rsdt,
            AcpiHeader::Xsdt(xsdt) => xsdt,
        }
    }

    /// The data address of this header's data.
    fn data_address(&self) -> usize {
        self.as_sdt().data_address()
    }
}

pub struct AcpiTable {
    header: AcpiHeader,
    entry_count: usize,
}

impl AcpiTable {
    /// Create a new ACPI table from the RSDP address.
    fn new(offset_table: &mut OffsetPageTable, rsdp_address: VirtAddr) -> Self {
        // SAFTEY: Safe to cast the RSDP address to the RSDP struct as the
        // address is verified by the bootloader.
        let rsdp = unsafe { &*(rsdp_address.as_u64() as *const Rsdp) };
        let sdt_address = rsdp.get_sdt_address() as u64;

        // SAFTEY: Already would have caused UB if the RSDP address was
        // anyhow invalid.
        let sdt = unsafe { Sdt::from_address(sdt_address, offset_table) };

        let sdt_signature = sdt.get_signature();
        let sdt_data_len = sdt.data_len();

        let (header, entry_count) = match sdt_signature {
            sdt::RSDT_SIGNATURE => (AcpiHeader::Rsdt(sdt), sdt_data_len / mem::size_of::<u32>()),
            sdt::XSDT_SIGNATURE => (AcpiHeader::Xsdt(sdt), sdt_data_len / mem::size_of::<u64>()),

            _ => panic!("Invalid ACPI header signature: {}", sdt_signature),
        };

        Self {
            header,
            entry_count,
        }
    }

    /// Lookup ACPI table entry with the provided signature.
    fn lookup_entry(
        &self,
        offset_table: &mut OffsetPageTable,
        signature: &str,
    ) -> Option<&'static Sdt> {
        let header_data_address = self.header.data_address() as *const u32;

        for i in 0..self.entry_count {
            // SAFTEY: Item address is valid as we are looping under the entry count and
            // the data address.
            let item_address = unsafe { *(header_data_address.add(i)) } as u64;
            let item = unsafe { Sdt::from_address(item_address, offset_table) };

            if item.get_signature() == signature {
                return Some(item);
            }
        }

        None
    }
}

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct GenericAddressStructure {
    pub address_space: u8,
    pub bit_width: u8,
    pub bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

impl GenericAddressStructure {
    pub unsafe fn init(&self, offset_table: &mut OffsetPageTable) {
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(self.address));
        let frame = PhysFrame::containing_address(PhysAddr::new(self.address));

        offset_table
            .map_to(
                page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
                &mut FRAME_ALLOCATOR,
            )
            .unwrap()
            .flush();
    }
}

/// Initialize the ACPI tables.
pub fn init(
    offset_table: &mut OffsetPageTable,
    rsdp_address: PhysAddr,
    physical_memory_offset: VirtAddr,
) {
    let rsdp_address = physical_memory_offset + rsdp_address.as_u64();
    let acpi_table = AcpiTable::new(offset_table, rsdp_address);

    macro init_table($sig:path => $ty:ty) {
        <$ty>::new(acpi_table.lookup_entry(offset_table, $sig));
    }

    if let Some(header) = acpi_table.lookup_entry(offset_table, mcfg::SIGNATURE) {
        unsafe {
            let mcfg: &'static Mcfg = header.as_ptr();
            mcfg.init();
        }
    }

    if let Some(header) = acpi_table.lookup_entry(offset_table, madt::SIGNATURE) {
        unsafe {
            // Not a valid MADT table without the local apic address and the flags.
            if header.data_len() < 8 {
                log::warn!(
                    "Assertion Failed: header.data_len() < 8 => {}",
                    header.data_len()
                );
            } else {
                let madt: &'static Madt = header.as_ptr();
                madt.init(offset_table).expect("Failed to initialize APIC");
            }
        }
    }

    init_table!(fadt::SIGNATURE => Fadt);

    Hpet::new(
        acpi_table.lookup_entry(offset_table, hpet::SIGNATURE),
        offset_table,
    );
}
