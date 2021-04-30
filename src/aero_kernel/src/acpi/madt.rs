use core::{
    alloc::{GlobalAlloc, Layout},
    intrinsics, mem,
    sync::atomic::Ordering,
};

use spin::Once;
use x86_64::{registers::control::Cr3, structures::paging::*, PhysAddr, VirtAddr};

use super::sdt::Sdt;
use crate::{apic, arch::interrupts, kernel_ap_startup, AERO_SYSTEM_ALLOCATOR};

use crate::apic::CPU_COUNT;

pub const SIGNATURE: &str = "APIC";
pub const TRAMPOLINE: u64 = 0x8000;

static MADT: Once<&'static Madt> = Once::new();
static TRAMPOLINE_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline"));

#[repr(C, packed)]
pub struct Trampoline {
    ap_ready: *mut u64,
    ap_cpu_id: *mut u64,
    ap_page_table: *mut u64,
    ap_stack_ptr: *mut u64,
    ap_code: *mut u64,
}

impl Trampoline {
    #[inline]
    fn new() -> Self {
        let ap_ready = (TRAMPOLINE + 8) as *mut u64;
        let ap_cpu_id = unsafe { ap_ready.offset(1) };
        let ap_page_table = unsafe { ap_ready.offset(2) };
        let ap_stack_ptr = unsafe { ap_ready.offset(3) };
        let ap_code = unsafe { ap_ready.offset(4) };

        Self {
            ap_ready,
            ap_cpu_id,
            ap_page_table,
            ap_stack_ptr,
            ap_code,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Madt {
    header: Sdt,
    local_apic_address: u32,
    flags: u32,
}

impl Madt {
    pub(super) fn init(
        &'static self,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        offset_table: &mut OffsetPageTable,
    ) {
        MADT.call_once(move || self);

        let trampoline_frame = PhysFrame::containing_address(PhysAddr::new(TRAMPOLINE));
        let trampoline_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(TRAMPOLINE));

        unsafe {
            offset_table
                .map_to(
                    trampoline_page,
                    trampoline_frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .unwrap()
                .flush();
        }

        log::debug!("Storing AP trampoline in {:#x}", TRAMPOLINE);

        // Atomic store the AP trampoline code to a fixed address in low conventional memory.
        unsafe {
            for i in 0..TRAMPOLINE_BIN.len() {
                intrinsics::atomic_store((TRAMPOLINE as *mut u8).add(i), TRAMPOLINE_BIN[i]);
            }
        }

        for entry in self.iter() {
            match entry {
                MadtEntry::LocalApic(local_apic) => {
                    if local_apic.apic_id == apic::get_bsp_id() as u8 {
                        // We do not want to start the BSP that is already running
                        // this code :D
                        continue;
                    }

                    if local_apic.flags & 1 != 1 {
                        // We cannot initialize disabled hardware :D
                        log::warn!("APIC {} is disabled by the hardware", local_apic.apic_id);
                    }

                    // Increase the CPU count.
                    CPU_COUNT.fetch_add(1, Ordering::SeqCst);

                    // Create the trampoline structure.
                    let trampoline = Trampoline::new();

                    unsafe {
                        let page_table = Cr3::read().0.start_address().as_u64();
                        let stack = AERO_SYSTEM_ALLOCATOR
                            .alloc(Layout::from_size_align_unchecked(4096 * 16, 4096))
                            .offset(4096 * 16) as u64;

                        intrinsics::atomic_store(trampoline.ap_ready, 0x00);
                        intrinsics::atomic_store(trampoline.ap_cpu_id, local_apic.apic_id as u64);
                        intrinsics::atomic_store(trampoline.ap_page_table, page_table);
                        intrinsics::atomic_store(trampoline.ap_stack_ptr, stack);
                        intrinsics::atomic_store(trampoline.ap_code, kernel_ap_startup as u64)
                    }

                    apic::mark_ap_ready(false);

                    let mut local_apic_init = apic::get_local_apic();

                    // Send init IPI to the local apic.
                    unsafe {
                        let mut icr = 0x4500;
                        icr |= (local_apic.apic_id as u64) << 56;

                        local_apic_init.set_icr(icr);
                    }

                    // unsafe {
                    //     let ap_segment = (TRAMPOLINE >> 12) & 0xFF;
                    //     let mut icr = 0x4600 | ap_segment as u64;

                    //     icr |= (local_apic.apic_id as u64) << 56;

                    //     local_apic_init.set_icr(icr);
                    // }

                    // unsafe {
                    //     // Wait for the AP to be ready.
                    //     while intrinsics::atomic_load(trampoline.ap_ready) == 0 {
                    //         interrupts::pause();
                    //     }
                    // }

                    // // Wait for the trampoline to be ready.
                    // while !apic::ap_ready() {
                    //     interrupts::pause();
                    // }

                    log::info!("Loaded multicore");
                }

                MadtEntry::IoApic(_) => {}
                MadtEntry::IntSrcOverride(_) => {}
            }
        }
    }

    fn iter(&self) -> MadtIterator {
        unsafe {
            MadtIterator {
                current: (self as *const Self as *const u8).add(mem::size_of::<Self>()),
                limit: (self as *const _ as *const u8).offset(self.header.length as isize),
            }
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct MadtEntryHeader {
    entry_type: u8,
    length: u8,
}

#[repr(C, packed)]
struct MadtLocalApic {
    header: MadtEntryHeader,
    processor_id: u8,
    apic_id: u8,
    flags: u32,
}

#[repr(C, packed)]
struct MadtIoApic {
    header: MadtEntryHeader,
    io_apic_id: u8,
    reserved: u8,
    io_apic_address: u32,
    global_system_interrupt_base: u32,
}

#[repr(C, packed)]
struct MadtIntSrcOverride {
    header: MadtEntryHeader,
    bus: u8,
    irq: u8,
    global_system_interrupt: u32,
    flags: u16,
}

enum MadtEntry {
    LocalApic(&'static MadtLocalApic),
    IoApic(&'static MadtIoApic),
    IntSrcOverride(&'static MadtIntSrcOverride),
}

struct MadtIterator {
    current: *const u8,
    limit: *const u8,
}

impl Iterator for MadtIterator {
    type Item = MadtEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.limit {
            unsafe {
                let entry_pointer = self.current;
                let header = *(self.current as *const MadtEntryHeader);

                self.current = self.current.offset(header.length as isize);

                let item = match header.entry_type {
                    0 => MadtEntry::LocalApic(&*(entry_pointer as *const MadtLocalApic)),
                    1 => MadtEntry::IoApic(&*(entry_pointer as *const MadtIoApic)),
                    2 => MadtEntry::IntSrcOverride(&*(entry_pointer as *const MadtIntSrcOverride)),

                    0x10..=0x7f => continue,
                    0x80..=0xff => continue,

                    _ => {
                        log::warn!("Unknown MADT entry with id: {}", header.entry_type);

                        return None;
                    }
                };

                return Some(item);
            }
        }

        None
    }
}
