use super::sdt::SDT;

/// The ACPI MCFG table describes the location of the PCI Express configuration space.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct MCFG {
    header: SDT,
    reserved: u64,
}
