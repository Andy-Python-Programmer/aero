//! This file contains the source for the GDT (Global Descriptor Table).
//! The GDT contains entries telling the CPU about memory segments.
//!
//! **Notes**: https://wiki.osdev.org/Global_Descriptor_Table

use lazy_static::lazy_static;

// TODO: Each GDT for every different arch.
// TODO: https://github.com/rust-lang/rust/issues/83107

/// The GDT Descriptor containing the size of offset of the table.
#[repr(packed)]
struct GDTDescriptor {
    /// The size of the table subtracted by 1.
    /// The size of the table is subtracted by 1 as the maximum value
    /// of `size` is 65535, while the GDT can be up to 65536 bytes.
    size: u16,
    /// The linear address of the table.
    offset: u64,
}

struct GDTEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access_byte: u8,
    /// Four bits of the variable is the limit and rest four bits of the
    /// variable are the flags.
    limit_hi_flags: u8,
    base_high: u8,
}

impl GDTEntry {
    fn new(
        limit_low: u16,
        base_low: u16,
        base_middle: u8,
        access_byte: u8,
        limit_hi_flags: u8,
        base_high: u8,
    ) -> Self {
        Self {
            limit_low,
            base_low,
            base_middle,
            access_byte,
            limit_hi_flags,
            base_high,
        }
    }
}

#[repr(align(0x1000))]
struct GDT {
    /// `0x00`
    kernel_null: GDTEntry,
    /// `0x08`
    kernel_code: GDTEntry,
    /// `0x10`
    kernel_data: GDTEntry,
    user_null: GDTEntry,
    user_code: GDTEntry,
    user_data: GDTEntry,
}

/// Initialize the GDT.
pub fn init() {}

lazy_static! {
    static ref GLOBAL_DESCRIPTOR_TABLE: GDT = GDT {
        kernel_null: GDTEntry::new(0, 0, 0, 0x00, 0x00, 0),
        kernel_code: GDTEntry::new(0, 0, 0, 0x9a, 0xa0, 0),
        kernel_data: GDTEntry::new(0, 0, 0, 0x92, 0xa0, 0),
        user_null: GDTEntry::new(0, 0, 0, 0x00, 0x00, 0),
        user_code: GDTEntry::new(0, 0, 0, 0x9a, 0xa0, 0),
        user_data: GDTEntry::new(0, 0, 0, 0x92, 0xa0, 0)
    };
}
