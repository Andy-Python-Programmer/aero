#[derive(Debug)]
#[repr(C)]
struct FdtHeader {
    magic: u32,
    totalsize: u32,
    off_dt_struct: u32,
    off_dt_strings: u32,
    off_mem_rsvmap: u32,
    version: u32,
    last_comp_version: u32,
    boot_cpuid_phys: u32,
    size_dt_strings: u32,
    size_dt_struct: u32,
}

pub struct Dtb<'a> {
    header: &'a FdtHeader,
}

impl<'a> Dtb<'a> {
    pub fn new(blob: *const u8) -> Self {
        let header = unsafe { &*(blob as *const FdtHeader) };
        // assert_eq!(header.magic, 0xd00dfeed);

        log::debug!("{header:#x?}");

        Self { header }
    }
}
