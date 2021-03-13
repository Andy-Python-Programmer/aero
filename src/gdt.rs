//! This file contains the source for the GDT (Global Descriptor Table).
//! The GDT contains entries telling the CPU about memory segments.
//!
//! **Notes**: https://wiki.osdev.org/Global_Descriptor_Table

use lazy_static::lazy_static;

// TODO: Each GDT for every different arch.

struct GlobalDescriptorTable {}

impl GlobalDescriptorTable {
    #[inline]
    pub fn new() -> Self {
        Self {}
    }
}

/// Initialize the GDT.
pub fn init() {}

lazy_static! {
    static ref GLOBAL_DESCRIPTOR_TABLE: GlobalDescriptorTable = {
        let table = GlobalDescriptorTable::new();

        table
    };
}
