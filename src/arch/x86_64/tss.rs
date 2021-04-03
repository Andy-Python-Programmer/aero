//! The Task State Segment (TSS) is a special data structure for x86 processors which holds information about a task.
//!
//! **Notes**: <https://wiki.osdev.org/Task_State_Segment>

use core::mem;

use super::gdt::GDTEntry;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct TSSEntry {
    /// The previous TSS - with hardware task switching these form a kind of backward linked list.
    previous_tss: u64,
    /// The stack pointer to load when changing to kernel mode.
    esp0: u64,
    /// The stack segment to load when changing to kernel mode.
    ss0: u64,
    esp1: u64,
    ss1: u64,
    esp2: u64,
    ss2: u64,
    cr3: u64,
    eip: u64,
    eflags: u64,
    eax: u64,
    ecx: u64,
    edx: u64,
    ebx: u64,
    esp: u64,
    ebp: u64,
    esi: u64,
    edi: u64,
    es: u64,
    cs: u64,
    ss: u64,
    ds: u64,
    fs: u64,
    gs: u64,
    ldt: u64,
    trap: u16,
    iomap_base: u16,
}

impl TSSEntry {
    #[inline]
    pub const fn null() -> Self {
        Self {
            previous_tss: 0,
            esp0: 0,
            ss0: 0,
            esp1: 0,
            ss1: 0,
            esp2: 0,
            ss2: 0,
            cr3: 0,
            eip: 0,
            eflags: 0,
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            es: 0,
            cs: 0,
            ss: 0,
            ds: 0,
            fs: 0,
            gs: 0,
            ldt: 0,
            trap: 0,
            iomap_base: mem::size_of::<Self>() as u16,
        }
    }

    pub fn new() -> (Self, GDTEntry) {
        let this = Self::null();

        let base = (&this as *const Self) as usize;
        let limit = base + mem::size_of::<Self>();

        (
            this,
            GDTEntry::new(0, 0, base as u8, limit as u8, 0xE9, 0x00),
        )
    }

    pub unsafe fn load(&self) {
        // TODO: Flush TSS.
    }
}
