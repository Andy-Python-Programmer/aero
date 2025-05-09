// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

//! The GDT contains entries telling the CPU about memory segments.
//!
//! In our case we only need a descriptor table in `x86_64` (as in every other
//! arch that Aero currently support) as those arch's do not implement segmentation
//! at all.
//!
//! ## Notes
//! * <https://wiki.osdev.org/Global_Descriptor_Table>

use core::alloc::Layout;
use core::ptr::addr_of;
use core::{mem, ptr};

use alloc::alloc::alloc_zeroed;

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone)]
    struct GdtEntryFlags: u8 {
        const PROTECTED_MODE = 1 << 6;
        const LONG_MODE = 1 << 5;
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum PrivilegeLevel {
    Ring0 = 0,
    Ring3 = 3,
}

impl PrivilegeLevel {
    pub fn is_user(&self) -> bool {
        matches!(self, Self::Ring3)
    }
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
    // GDT user data descriptor. (used by SYSCALL)
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
    // GDT user data descriptor. (used by SYSENTER)
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_3
            | GdtAccessFlags::SYSTEM
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
        GdtEntryFlags::empty(),
    ),
    // GDT null descriptor as the TSS should be 16 bytes long
    // and twice the normal size.
    GdtEntry::NULL,
];

struct GdtAccessFlags;

impl GdtAccessFlags {
    const EXECUTABLE: u8 = 1 << 3;
    const NULL: u8 = 0;
    const PRESENT: u8 = 1 << 7;
    const PRIVILEGE: u8 = 1 << 1;
    const RING_0: u8 = 0 << 5;
    const RING_3: u8 = 3 << 5;
    const SYSTEM: u8 = 1 << 4;
    const TSS_AVAIL: u8 = 9;
}

pub struct GdtEntryIndex;

#[rustfmt::skip]
impl GdtEntryIndex {
    pub const KERNEL_CODE: u16 = 1;
    pub const KERNEL_DATA: u16 = 2;
    pub const KERNEL_TLS: u16 = 3;
    pub const USER_DATA: u16 = 4;
    pub const USER_CODE: u16 = 5;
    pub const TSS: u16 = 8;
    pub const TSS_HI: u16 = 9;
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct SegmentSelector(u16);

impl SegmentSelector {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn new(index: u16, privilege_level: PrivilegeLevel) -> Self {
        Self(index << 3 | (privilege_level as u16))
    }

    pub const fn bits(&self) -> u16 {
        self.0
    }

    pub const fn from_bits(value: u16) -> Self {
        Self(value)
    }

    pub const fn privilege_level(&self) -> PrivilegeLevel {
        match self.bits() & 0b11 {
            0 => PrivilegeLevel::Ring0,
            3 => PrivilegeLevel::Ring3,
            _ => unreachable!(),
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
    const NULL: Self = Self::new(GdtAccessFlags::NULL, GdtEntryFlags::empty());

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
            *(ptr::addr_of_mut!(*self).cast::<T>()) = value;
        }
    }
}

/// Although hardware task-switching is not supported in 64-bit mode, a task
/// state segment is required since the it holds information important to 64-bit
/// mode and its not directly related to task switching mechanism.
#[repr(C, packed)]
pub struct Tss {
    reserved: u32, // offset 0x00

    /// The full 64-bit canonical forms of the stack pointers (RSP) for
    /// privilege levels 0-2.
    pub rsp: [u64; 3], // offset 0x04
    pub reserved2: u64, // offset 0x1C

    /// The full 64-bit canonical forms of the interrupt stack table
    /// (IST) pointers.
    pub ist: [u64; 7], // offset 0x24
    reserved3: u64, // offset 0x5c
    reserved4: u16, // offset 0x64

    /// The 16-bit offset to the I/O permission bit map from the 64-bit
    /// TSS base.
    pub iomap_base: u16, // offset 0x66
}

#[cpu_local(subsection = "tss")]
pub static mut TSS: Tss = Tss {
    reserved: 0,
    rsp: [0; 3],
    reserved2: 0,
    ist: [0; 7],
    reserved3: 0,
    reserved4: 0,
    iomap_base: 0,
};

/// Initialize the bootstrap GDT which is required to initialize TLS (Thread Local Storage)
/// support so, after the kernel heap we will map the TLS section and initialize the *actual* GDT
/// and then each CPU will have it's own GDT but we only will have to define it once as a
/// `#[thread_local]`.
pub fn init_boot() {
    unsafe {
        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; BOOT_GDT_ENTRY_COUNT]>() - 1) as u16,
            addr_of!(BOOT_GDT).addr() as u64,
        );

        load_gdt(&gdt_descriptor);
    }

    // Load the GDT segments.
    unsafe {
        load_cs(SegmentSelector::new(
            GdtEntryIndex::KERNEL_CODE,
            PrivilegeLevel::Ring0,
        ));

        load_ds(SegmentSelector::new(
            GdtEntryIndex::KERNEL_DATA,
            PrivilegeLevel::Ring0,
        ));

        load_es(SegmentSelector::new(
            GdtEntryIndex::KERNEL_DATA,
            PrivilegeLevel::Ring0,
        ));

        load_fs(SegmentSelector::new(
            GdtEntryIndex::KERNEL_DATA,
            PrivilegeLevel::Ring0,
        ));

        load_gs(SegmentSelector::new(
            GdtEntryIndex::KERNEL_TLS,
            PrivilegeLevel::Ring0,
        ));

        load_ss(SegmentSelector::new(
            GdtEntryIndex::KERNEL_DATA,
            PrivilegeLevel::Ring0,
        ));
    }
}

static STK: [u8; 4096 * 16] = [0; 4096 * 16];

pub const USER_SS: SegmentSelector =
    SegmentSelector::new(GdtEntryIndex::USER_DATA, PrivilegeLevel::Ring3);

pub const USER_CS: SegmentSelector =
    SegmentSelector::new(GdtEntryIndex::USER_CODE, PrivilegeLevel::Ring3);

/// Initialize the *actual* GDT stored in TLS.
///
/// ## Safety
/// The heap must be initialized before this function is called.
pub fn init() {
    let gdt = unsafe {
        let gdt_ent_size = core::mem::size_of::<GdtEntry>();
        let gdt_ent_align = core::mem::align_of::<GdtEntry>();

        let gdt_size = gdt_ent_size * GDT_ENTRY_COUNT;
        let layout = Layout::from_size_align_unchecked(gdt_size, gdt_ent_align);

        let ptr = alloc_zeroed(layout).cast::<GdtEntry>();
        core::slice::from_raw_parts_mut::<GdtEntry>(ptr, GDT_ENTRY_COUNT)
    };

    // Copy over the GDT template:
    gdt.copy_from_slice(&GDT);

    unsafe {
        let tss_ptr = TSS.addr().as_mut_ptr::<Tss>();

        gdt[GdtEntryIndex::TSS as usize].set_offset(tss_ptr as u32);
        gdt[GdtEntryIndex::TSS as usize].set_limit(mem::size_of::<Tss>() as u32);
        gdt[GdtEntryIndex::TSS_HI as usize].set_raw((tss_ptr as u64) >> 32);

        TSS.rsp[0] = STK.as_ptr().offset(4096 * 16) as u64;

        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; GDT_ENTRY_COUNT]>() - 1) as u16,
            gdt.as_ptr() as u64,
        );

        load_gdt(&gdt_descriptor);

        // Reload the GDT segments.
        load_cs(SegmentSelector::new(
            GdtEntryIndex::KERNEL_CODE,
            PrivilegeLevel::Ring0,
        ));
        load_ds(SegmentSelector::new(
            GdtEntryIndex::KERNEL_DATA,
            PrivilegeLevel::Ring0,
        ));
        load_es(SegmentSelector::new(
            GdtEntryIndex::KERNEL_DATA,
            PrivilegeLevel::Ring0,
        ));
        load_ss(SegmentSelector::new(
            GdtEntryIndex::KERNEL_DATA,
            PrivilegeLevel::Ring0,
        ));

        // Load the Task State Segment.
        load_tss(SegmentSelector::new(
            GdtEntryIndex::TSS,
            PrivilegeLevel::Ring0,
        ));
    }

    // // Now we update the per-cpu storage to store a reference
    // // to the per-cpu GDT.
    // tls::get_percpu().gdt = gdt;
}

#[inline(always)]
unsafe fn load_cs(selector: SegmentSelector) {
    // NOTE: We cannot directly move into CS since x86 requires the IP
    // and CS set at the same time. To do this, we need push the new segment
    // selector and return value onto the stack and far return to reload CS and
    // continue execution.
    //
    // We also cannot use a far call or a far jump since we would only be
    // able to jump to 32-bit instruction pointers. Only Intel supports for
    // 64-bit far calls/jumps in long-mode, AMD does not.
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
