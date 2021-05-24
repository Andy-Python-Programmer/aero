//! The GDT contains entries telling the CPU about memory segments.
//!
//! In our case we only need a descriptor table in `x86_64` (as in every other
//! arch that Aero currently support) as those arch's do not implement segmentation
//! at all.
//!
//! ## Notes
//! * <https://wiki.osdev.org/Global_Descriptor_Table>

#![cfg(target_arch = "x86_64")]

use core::mem;

use x86_64::VirtAddr;

use crate::mem::pti::{PTI_CPU_STACK, PTI_STACK_SIZE};
use crate::utils::io;

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

bitflags::bitflags! {
    struct GdtEntryFlags: u8 {
        const NULL = 0;
        const PROTECTED_MODE = 1 << 6;
        const LONG_MODE = 1 << 5;
    }
}

pub enum Ring {
    Ring0 = 0b00,
}

const BOOT_GDT_ENTRY_COUNT: usize = 4;
const GDT_ENTRY_COUNT: usize = 10;

static mut BOOT_GDT: [GdtEntry; BOOT_GDT_ENTRY_COUNT] = [
    // GDT null descriptor.
    GdtEntry::NULL,
    // GDT kernel code descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::EXECUTABLE
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT kernel data descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT kernel TLS descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
];

#[thread_local]
static mut GDT: [GdtEntry; GDT_ENTRY_COUNT] = [
    // GDT null descriptor.
    GdtEntry::NULL,
    // GDT kernel code descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::EXECUTABLE
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT kernel data descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT kernel TLS descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT dummy user code descriptor. Required for SYSEXIT.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::EXECUTABLE
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::PROTECTED_MODE,
    ),
    // GDT user data descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_3
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT user code descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_3
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::EXECUTABLE
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT user TLS descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_3
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT TSS descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT | GdtAccessFlags::RING_3 | GdtAccessFlags::TSS_AVAIL,
        GdtEntryFlags::NULL,
    ),
    // GDT null descriptor as the TSS should be 16 bytes long
    // and twice the normal size.
    GdtEntry::NULL,
];

#[thread_local]
pub static mut TASK_STATE_SEGMENT: Tss = Tss::new();

struct GdtAccessFlags;

impl GdtAccessFlags {
    const NULL: u8 = 0;
    const PRESENT: u8 = 1 << 7;
    const RING_0: u8 = 0 << 5;
    const RING_3: u8 = 3 << 5;
    const SYSTEM: u8 = 1 << 4;
    const EXECUTABLE: u8 = 1 << 3;
    const PRIVILEGE: u8 = 1 << 1;
    const TSS_AVAIL: u8 = 9;
}

pub struct GdtEntryType;

impl GdtEntryType {
    pub const KERNEL_CODE: u16 = 1;
    pub const KERNEL_DATA: u16 = 2;
    pub const KERNEL_TLS: u16 = 3;
    pub const USER_CODE32_UNUSED: u16 = 4;
    pub const TSS: u16 = 8;
    pub const TSS_HI: u16 = 9;
}

impl SegmentSelector {
    const fn new(index: u16, rpl: Ring) -> Self {
        Self {
            bits: index << 3 | (rpl as u16),
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
struct GdtEntry {
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
    const NULL: Self = Self::new(GdtAccessFlags::NULL, GdtEntryFlags::NULL);

    const fn new(access_flags: u8, entry_flags: GdtEntryFlags) -> Self {
        Self {
            limit_low: 0x00,
            base_low: 0x00,
            base_middle: 0x00,
            access_byte: access_flags,
            limit_hi_flags: entry_flags.bits() & 0xF0,
            base_hi: 0x00,
        }
    }

    fn set_offset(&mut self, offset: u32) {
        self.base_low = offset as u16;
        self.base_middle = (offset >> 16) as u8;
        self.base_hi = (offset >> 24) as u8;
    }

    fn set_limit(&mut self, limit: u32) {
        self.limit_low = limit as u16;
        self.limit_hi_flags = self.limit_hi_flags & 0xF0 | ((limit >> 16) as u8) & 0x0F;
    }

    fn set_raw<T>(&mut self, value: T) {
        unsafe {
            (self as *mut _ as *mut T).write(value);
        }
    }
}

/// The Task State Segment (TSS) is a special data structure for x86 processors which holds information about a task.
///
/// **Notes**: <https://wiki.osdev.org/Task_State_Segment>
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Tss {
    reserved: u32,

    /// The full 64-bit canonical forms of the stack pointers (RSP) for privilege levels 0-2.
    rsp: [u64; 3],
    reserved2: u64,

    /// The full 64-bit canonical forms of the interrupt stack table (IST) pointers.
    ist: [u64; 7],
    reserved3: u64,
    reserved4: u16,

    /// The 16-bit offset to the I/O permission bit map from the 64-bit TSS base.
    iomap_base: u16,
}

impl Tss {
    #[inline]
    const fn new() -> Self {
        Self {
            reserved: 0,
            rsp: [0; 3],
            reserved2: 0,
            ist: [0; 7],
            reserved3: 0,
            reserved4: 0,
            iomap_base: 0xFFFF,
        }
    }
}

/// Initialize the bootstrap GDT which is required to initialize TLS (Thread Local Storage)
/// support so, after the kernel heap we will map the TLS section and initialize the *actual* GDT
/// and then each CPU will have it's own GDT but we only will have to define it once as a `#[thread_local]`.
pub fn init_boot() {
    unsafe {
        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; BOOT_GDT_ENTRY_COUNT]>() - 1) as u16,
            (&BOOT_GDT as *const _) as u64,
        );

        load_gdt(&gdt_descriptor as *const _);
    }

    // Load the GDT segments.
    unsafe {
        load_cs(SegmentSelector::new(GdtEntryType::KERNEL_CODE, Ring::Ring0));
        load_ds(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_es(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_fs(SegmentSelector::new(GdtEntryType::KERNEL_TLS, Ring::Ring0));
        load_gs(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_ss(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
    }
}

/// Initialize the *actual* GDT stored in TLS.
pub fn init(stack_top: VirtAddr) {
    unsafe {
        let tss_ptr = &mut TASK_STATE_SEGMENT as *mut Tss;

        GDT[GdtEntryType::TSS as usize].set_offset(tss_ptr as u32);
        GDT[GdtEntryType::TSS as usize].set_limit(mem::size_of::<Tss>() as u32);
        GDT[GdtEntryType::TSS_HI as usize].set_raw((tss_ptr as u64) >> 32);

        let init_stack_addr = PTI_CPU_STACK.as_ptr() as usize + PTI_STACK_SIZE;

        TASK_STATE_SEGMENT.rsp[0] = init_stack_addr as _;
        TASK_STATE_SEGMENT.rsp[0] = stack_top.as_u64();

        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; GDT_ENTRY_COUNT]>() - 1) as u16,
            (&GDT as *const _) as u64,
        );

        load_gdt(&gdt_descriptor as *const _);

        io::wrmsr(io::IA32_KERNEL_GSBASE, tss_ptr as *mut _ as u64);

        // Reload the GDT segments.
        load_cs(SegmentSelector::new(GdtEntryType::KERNEL_CODE, Ring::Ring0));
        load_ds(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_es(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_ss(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));

        // Load the Task State Segment.
        load_tss(SegmentSelector::new(GdtEntryType::TSS, Ring::Ring0));
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
unsafe fn load_tss(selector: SegmentSelector) {
    asm!("ltr {0:x}", in(reg) selector.bits(), options(nostack, nomem));
}

#[inline(always)]
unsafe fn load_gdt(gdt_descriptor: *const GdtDescriptor) {
    asm!("lgdt [{}]", in(reg) gdt_descriptor, options(nostack));
}
