//! The GDT contains entries telling the CPU about memory segments.
//!
//! **Notes**: <https://wiki.osdev.org/Global_Descriptor_Table>

use core::mem;

const GDT_ENTRIES: usize = 6;

static mut GDT: [GdtEntry; GDT_ENTRIES] = [GdtEntry::NULL; GDT_ENTRIES];

bitflags::bitflags! {
    /// Specifies which element to load into a segment from
    /// descriptor tables (i.e., is a index to LDT or GDT table
    /// with some additional flags).
    pub struct SegmentSelector: u16 {
        const RPL_0 = 0b00;
        const RPL_1 = 0b01;
        const RPL_2 = 0b10;
        const RPL_3 = 0b11;
        const TI_GDT = 0 << 2;
        const TI_LDT = 1 << 2;
    }
}

impl SegmentSelector {
    #[inline(always)]
    const fn new(index: u16, rpl: Self) -> Self {
        Self {
            bits: index << 3 | rpl.bits,
        }
    }
}

#[repr(C, packed)]
struct GdtDescriptor {
    /// The size of the table subtracted by 1.
    /// The size of the table is subtracted by 1 as the maximum value
    /// of `size` is 65535, while the GDT can be up to 65536 bytes.
    size: u16,
    /// The linear address of the table.
    offset: u64,
}

impl GdtDescriptor {
    /// Create a new GDT descriptor.
    #[inline]
    pub const fn new(size: u16, offset: u64) -> Self {
        Self { size, offset }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GdtEntry {
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

impl GdtEntry {
    const NULL: Self = Self::new(0x00, 0x00, 0x00, 0x00, 0x00, 0x00);

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
}

/// The Task State Segment (TSS) is a special data structure for x86 processors which holds information about a task.
///
/// **Notes**: <https://wiki.osdev.org/Task_State_Segment>
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct TssEntry {
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

impl TssEntry {
    #[inline]
    pub const fn new() -> Self {
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

    pub fn as_gdt_entry(self) -> GdtEntry {
        let base = (&self as *const Self) as usize;
        let limit = base + mem::size_of::<Self>();

        GdtEntry::new(0, 0, base as u8, limit as u8, 0xE9, 0x00)
    }
}

/// Initialize the GDT.
pub fn init() {
    unsafe {
        let tss = TssEntry::new().as_gdt_entry();

        GDT[0] = GdtEntry::new(0, 0, 0, 0x00, 0x00, 0);
        GDT[1] = GdtEntry::new(0, 0, 0, 0x9A, 0xA0, 0);
        GDT[2] = GdtEntry::new(0, 0, 0, 0x92, 0xA0, 0);
        GDT[3] = GdtEntry::new(0, 0, 0, 0xFA, 0xA0, 0);
        GDT[4] = GdtEntry::new(0, 0, 0, 0xF2, 0xA0, 0);
        GDT[5] = tss;

        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; GDT_ENTRIES]>() - 1) as u16,
            (&GDT as *const _) as u64,
        );

        load_gdt(&gdt_descriptor as *const _);

        // Reload the GDT segments.
        load_cs(SegmentSelector::new(1, SegmentSelector::RPL_0));
        load_ds(SegmentSelector::new(2, SegmentSelector::RPL_0));
        load_es(SegmentSelector::new(2, SegmentSelector::RPL_0));
        load_fs(SegmentSelector::new(2, SegmentSelector::RPL_0));
        load_gs(SegmentSelector::new(3, SegmentSelector::RPL_0));
        load_ss(SegmentSelector::new(2, SegmentSelector::RPL_0));

        load_tss(&tss as *const _);
    }
}

#[inline(always)]
unsafe fn load_cs(selector: SegmentSelector) {
    asm!(
        "push {selector}",
        "lea {tmp}, [1f + rip]",
        "push {tmp}",
        "retfq",
        "1:",
        selector = in(reg) u64::from(selector.bits()),
        tmp = lateout(reg) _,
    );
}

#[inline(always)]
unsafe fn load_ds(selector: SegmentSelector) {
    asm!("mov ds, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_es(selector: SegmentSelector) {
    asm!("mov es, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_fs(selector: SegmentSelector) {
    asm!("mov fs, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_gs(selector: SegmentSelector) {
    asm!("mov gs, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_ss(selector: SegmentSelector) {
    asm!("mov ss, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_gdt(gdt_descriptor: *const GdtDescriptor) {
    asm!("lgdt [rdi]", in("rdi") gdt_descriptor)
}

#[inline(always)]
unsafe fn load_tss(gdt_entry: *const GdtEntry) {
    asm!("ltr [rdi]", in("rdi") gdt_entry)
}
