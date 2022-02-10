/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

//! The GDT contains entries telling the CPU about memory segments.
//!
//! In our case we only need a descriptor table in `x86_64` (as in every other
//! arch that Aero currently support) as those arch's do not implement segmentation
//! at all.
//!
//! ## Notes
//! * <https://wiki.osdev.org/Global_Descriptor_Table>

use core::alloc::Layout;
use core::mem;

use alloc::alloc::alloc_zeroed;

use crate::mem::paging::VirtAddr;

use crate::arch::tls::PerCpuData;

use super::tls;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ring {
    Ring0 = 0b00,
    Ring3 = 0b11,
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

static GDT: [GdtEntry; GDT_ENTRY_COUNT] = [
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
pub(super) struct GdtEntry {
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
            *(self as *mut _ as *mut T) = value;
        }
    }
}

/// Although hardware task-switching is not supported in 64-bit mode, a task
/// state segment is required since the it holds information important to 64-bit
/// mode and its not directly related to task switching mechanism.
#[repr(C, packed)]
pub struct Tss {
    reserved: u32,

    /// The full 64-bit canonical forms of the stack pointers (RSP) for
    /// privilege levels 0-2.
    pub rsp: [u64; 3],
    reserved2: u64,

    /// The full 64-bit canonical forms of the interrupt stack table
    /// (IST) pointers.
    pub ist: [u64; 7],
    reserved3: u64,
    reserved4: u16,

    /// The 16-bit offset to the I/O permission bit map from the 64-bit
    /// TSS base.
    pub iomap_base: u16,
}

// Processor Control Region
#[repr(C, packed)]
pub struct Kpcr {
    pub tss: Tss,
    pub cpu_local: &'static mut PerCpuData,
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

        load_gdt(&gdt_descriptor);
    }

    // Load the GDT segments.
    unsafe {
        load_cs(SegmentSelector::new(GdtEntryType::KERNEL_CODE, Ring::Ring0));
        load_ds(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_es(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_fs(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_gs(SegmentSelector::new(GdtEntryType::KERNEL_TLS, Ring::Ring0));
        load_ss(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
    }
}

/// SAFETY: The GS base should point to the kernel PCR.
pub fn get_task_state_segement() -> &'static mut Tss {
    unsafe { &mut *(io::rdmsr(io::IA32_GS_BASE) as *mut Tss) }
}

/// SAFETY: The GS base should point to the kernel PCR.
pub fn get_kpcr() -> &'static mut Kpcr {
    unsafe { &mut *(io::rdmsr(io::IA32_GS_BASE) as *mut Kpcr) }
}

/// Initialize the *actual* GDT stored in TLS.
///
/// ## Saftey
/// The heap must be initialized before this function is called.
pub fn init(stack_top: VirtAddr) {
    let gdt = unsafe {
        let gdt_ent_size = core::mem::size_of::<GdtEntry>();
        let gdt_ent_align = core::mem::align_of::<GdtEntry>();

        let gdt_size = gdt_ent_size * GDT_ENTRY_COUNT;
        let layout = Layout::from_size_align_unchecked(gdt_size, gdt_ent_align);

        let ptr = alloc_zeroed(layout) as *mut GdtEntry;
        core::slice::from_raw_parts_mut::<GdtEntry>(ptr, GDT_ENTRY_COUNT)
    };

    // Copy over the GDT template:
    gdt.copy_from_slice(&GDT);

    unsafe {
        let tss_ref = get_task_state_segement();
        let tss_ptr = tss_ref as *mut Tss;

        gdt[GdtEntryType::TSS as usize].set_offset(tss_ptr as u32);
        gdt[GdtEntryType::TSS as usize].set_limit(mem::size_of::<Tss>() as u32);
        gdt[GdtEntryType::TSS_HI as usize].set_raw((tss_ptr as u64) >> 32);

        tss_ref.rsp[0] = stack_top.as_u64();

        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; GDT_ENTRY_COUNT]>() - 1) as u16,
            gdt.as_ptr() as u64,
        );

        load_gdt(&gdt_descriptor);

        // Reload the GDT segments.
        load_cs(SegmentSelector::new(GdtEntryType::KERNEL_CODE, Ring::Ring0));
        load_ds(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_es(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));
        load_ss(SegmentSelector::new(GdtEntryType::KERNEL_DATA, Ring::Ring0));

        // Load the Task State Segment.
        load_tss(SegmentSelector::new(GdtEntryType::TSS, Ring::Ring0));
    }

    // Now we update the per-cpu storage to store a reference
    // to the per-cpu GDT.
    tls::get_percpu().gdt = gdt;
}

#[inline(always)]
unsafe fn load_cs(selector: SegmentSelector) {
    /*
     * NOTE: We cannot directly move into CS since x86 requires the IP
     * and CS set at the same time. To do this, we need push the new segment
     * selector and return value onto the stack and far return to reload CS and
     * continue execution.
     *
     * We also cannot use a far call or a far jump since we would only be
     * able to jump to 32-bit instruction pointers. Only Intel supports for
     * 64-bit far calls/jumps in long-mode, AMD does not.
     */
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
unsafe fn load_gdt(gdt_descriptor: &GdtDescriptor) {
    asm!("lgdt [{}]", in(reg) gdt_descriptor, options(nostack));
}
