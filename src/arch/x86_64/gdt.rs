//! The GDT contains entries telling the CPU about memory segments.
//!
//! **Notes**: <https://wiki.osdev.org/Global_Descriptor_Table>

use core::mem;

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

/// The Task State Segment (TSS) is a special data structure for x86 processors which holds information about a task.
///
/// **Notes**: <https://wiki.osdev.org/Task_State_Segment>
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

    pub fn new() -> GDTEntry {
        let this = Self::null();

        let base = (&this as *const Self) as usize;
        let limit = base + mem::size_of::<Self>();

        GDTEntry::new(0, 0, base as u8, limit as u8, 0xE9, 0x00)
    }
}

/// Initialize the GDT.
pub fn init() {
    unsafe {
        let tss = TSSEntry::new();

        GDT[0] = GDTEntry::new(0, 0, 0, 0x00, 0x00, 0);
        GDT[1] = GDTEntry::new(0, 0, 0, 0x9A, 0xA0, 0);
        GDT[2] = GDTEntry::new(0, 0, 0, 0x92, 0xA0, 0);
        GDT[3] = GDTEntry::new(0, 0, 0, 0xFA, 0xA0, 0);
        GDT[4] = GDTEntry::new(0, 0, 0, 0xF2, 0xA0, 0);
        GDT[5] = tss;

        let gdt_descriptor = GDTDescriptor::new(
            (mem::size_of::<[GDTEntry; GDT_ENTRIES]>() - 1) as u16,
            (&GDT as *const _) as u64,
        );

        load_gdt(&gdt_descriptor as *const _);
        load_tss(&tss as *const _);
    }
}

unsafe fn load_tss(gdt_entry: *const GDTEntry) {
    asm!("ltr [rdi]", in("rdi") gdt_entry)
}

extern "C" {
    fn load_gdt(gdt_descriptor: *const GDTDescriptor);
}
