use crate::acpi::fadt;
use crate::acpi::get_acpi_table;

use crate::mem::paging::PhysAddr;

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
}

pub fn init_lai() {
    let lai_host = box LaiHost;
    lai::init(lai_host);

    unsafe {
        lai::lai_set_acpi_revision(get_acpi_table().revision() as _);
        lai::lai_create_namespace();
    }
}

crate::module_init!(init_lai);
