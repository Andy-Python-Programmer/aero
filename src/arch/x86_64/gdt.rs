//! The GDT contains entries telling the CPU about memory segments.
//!
//! **Notes**: <https://wiki.osdev.org/Global_Descriptor_Table>

use core::intrinsics::size_of;

use super::tss::TSSEntry;

global_asm!(include_str!("load_gdt.s"));

const GDT_ENTRIES: usize = 6;

static mut GDT: [GDTEntry; GDT_ENTRIES] = [GDTEntry::null(); GDT_ENTRIES];

#[repr(C, packed)]
struct GDTDescriptor {
    /// The size of the table subtracted by 1.
    /// The size of the table is subtracted by 1 as the maximum value
    /// of `size` is 65535, while the GDT can be up to 65536 bytes.
    size: u16,
    /// The linear address of the table.
    offset: u64,
}

impl GDTDescriptor {
    /// Create a new GDT descriptor.
    #[inline]
    pub const fn new(size: u16, offset: u64) -> Self {
        Self { size, offset }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GDTEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access_byte: u8,
    /// The limit high and the flags.
    ///
    /// **Note**: Four bits of the variable is the limit and rest four bits of the
    /// variable are the flags.
    limit_hi_flags: u8,
    base_hi: u8,
}

impl GDTEntry {
    /// Create a new GDT entry.
    #[inline]
    pub const fn new(
        limit_low: u16,
        base_low: u16,
        base_middle: u8,
        access_byte: u8,
        limit_hi_flags: u8,
        base_hi: u8,
    ) -> Self {
        Self {
            limit_low,
            base_low,
            base_middle,
            access_byte,
            limit_hi_flags,
            base_hi,
        }
    }

    #[inline]
    const fn null() -> Self {
        Self::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
    }
}

/// Initialize the GDT.
pub fn init() {
    unsafe {
        let (tss, tss_gdt) = TSSEntry::new();

        GDT[0] = GDTEntry::new(0, 0, 0, 0x00, 0x00, 0);
        GDT[1] = GDTEntry::new(0, 0, 0, 0x9a, 0xa0, 0);
        GDT[2] = GDTEntry::new(0, 0, 0, 0x92, 0xa0, 0);
        GDT[3] = GDTEntry::new(0, 0, 0, 0xfa, 0xa0, 0);
        GDT[4] = GDTEntry::new(0, 0, 0, 0xf2, 0xa0, 0);
        GDT[5] = tss_gdt;

        let gdt_descriptor = GDTDescriptor::new(
            (size_of::<[GDTEntry; GDT_ENTRIES]>() - 1) as u16,
            (&GDT as *const _) as u64,
        );

        LoadGDT(&gdt_descriptor as *const _);
    }
}

extern "C" {
    fn LoadGDT(gdt_descriptor: *const GDTDescriptor);
}
