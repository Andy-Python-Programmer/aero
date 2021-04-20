use super::sdt::Sdt;

/// The ACPI MCFG table describes the location of the PCI Express configuration space.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct Mcfg {
    header: Sdt,
    reserved: u64,
}
