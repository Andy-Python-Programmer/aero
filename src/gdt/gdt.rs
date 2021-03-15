//! This file contains the source for the GDT (Global Descriptor Table).
//! The GDT contains entries telling the CPU about memory segments.
//!
//! **Notes**: https://wiki.osdev.org/Global_Descriptor_Table

use core::intrinsics::size_of;

use lazy_static::lazy_static;

// TODO: https://github.com/rust-lang/rust/issues/83107

/// The GDT Descriptor containing the size of offset of the table.
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
    pub fn new(size: u16, offset: u64) -> Self {
        Self { size, offset }
    }
}

/// A GDT entry.
#[repr(C)]
struct GDTEntry {
    /// Limit low.
    limit_low: u16,
    /// Base low.
    base_low: u16,
    /// Base middle.
    base_middle: u8,
    /// The access byte.
    access_byte: u8,
    /// The limit high and the flags.
    ///
    /// **Note**: Four bits of the variable is the limit and rest four bits of the
    /// variable are the flags.
    limit_hi_flags: u8,
    /// Base high.
    base_hi: u8,
}

impl GDTEntry {
    /// Create a new GDT entry.
    #[inline]
    fn new(
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
}

/// The GDT.
#[repr(C, align(0x1000))]
struct GDT {
    /// The kernel null segment: `0x00`.
    kernel_null: GDTEntry,
    /// The kernel code segment: `0x08`.
    kernel_code: GDTEntry,
    /// The kernel data segment: `0x10`.
    kernel_data: GDTEntry,
    /// The user null segment.
    user_null: GDTEntry,
    /// The user code segment.
    user_code: GDTEntry,
    /// The user data segment.
    user_data: GDTEntry,
}

/// Initialize the GDT.
pub fn init() {
    unsafe {
        let gdt_descriptor = GDTDescriptor::new(
            (size_of::<GDT>() - 1) as u16,
            (&GLOBAL_DESCRIPTOR_TABLE as *const _) as u64,
        );

        load_gdt(&gdt_descriptor as *const _)
    }
}

lazy_static! {
    /// The GDT (Global Descriptor Table).
    static ref GLOBAL_DESCRIPTOR_TABLE: GDT = GDT {
        kernel_null: GDTEntry::new(0, 0, 0, 0x00, 0x00, 0),
        kernel_code: GDTEntry::new(0, 0, 0, 0x9a, 0xa0, 0),
        kernel_data: GDTEntry::new(0, 0, 0, 0x92, 0xa0, 0),
        user_null: GDTEntry::new(0, 0, 0, 0x00, 0x00, 0),
        user_code: GDTEntry::new(0, 0, 0, 0x9a, 0xa0, 0),
        user_data: GDTEntry::new(0, 0, 0, 0x92, 0xa0, 0)
    };
}

/// Load the GDT using inline assembly.
unsafe fn load_gdt(gdt_descriptor: *const GDTDescriptor) {
    asm!(include_str!("./load_gdt.asm"), in(reg) gdt_descriptor, options(nostack));
}
