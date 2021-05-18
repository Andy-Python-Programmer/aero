use core::{intrinsics, mem, sync::atomic::Ordering};

use spin::Once;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{mapper::MapToError, *},
    PhysAddr, VirtAddr,
};

use crate::prelude::*;

use super::sdt::Sdt;
use crate::{
    apic::{self, IoApicHeader},
    kernel_ap_startup,
    mem::{alloc::malloc_align, paging::FRAME_ALLOCATOR},
};

use crate::apic::CPU_COUNT;

pub(super) const SIGNATURE: &str = "APIC";

const_unsafe! {
    const TRAMPOLINE_VIRTUAL: VirtAddr = VirtAddr::new_unsafe(0x8000);
    const TRAMPOLINE_PHYSICAL: PhysAddr = PhysAddr::new_unsafe(0x8000);
}

static MADT: Once<&'static Madt> = Once::new();
static TRAMPOLINE_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/trampoline"));

#[derive(Clone, Copy, Debug)]
pub struct Madt {
    header: Sdt,
    local_apic_address: u32,
    flags: u32,
}

impl Madt {
    pub(super) fn init(
        &'static self,
        offset_table: &mut OffsetPageTable,
    ) -> Result<(), MapToError<Size4KiB>> {
        MADT.call_once(move || self);

        let trampoline_frame: PhysFrame = PhysFrame::containing_address(TRAMPOLINE_PHYSICAL);
        let trampoline_page: Page = Page::containing_address(TRAMPOLINE_VIRTUAL);

        /*
         * Identity map the trampoline frame and make it writable.
         *
         * NOTE: Rather then using the identity_map function in the mapper
         * struct we are using the map_to function as we will be unmapping the
         * frame when we are done and that should save one call.
         */
        unsafe {
            offset_table.map_to(
                trampoline_page,
                trampoline_frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                &mut FRAME_ALLOCATOR,
            )
        }?
        .flush();

        log::debug!("Storing AP trampoline in {:#x}", TRAMPOLINE_VIRTUAL);

        /*
         * Atomic store the AP trampoline code and page table to a fixed address
         * in low conventional memory.
         */
        unsafe {
            for i in 0..TRAMPOLINE_BIN.len() {
                intrinsics::atomic_store(
                    TRAMPOLINE_VIRTUAL.as_mut_ptr::<u8>().add(i),
                    TRAMPOLINE_BIN[i],
                );
            }
        }

        for entry in self.iter() {
            match entry {
                MadtEntry::LocalApic(local_apic) => {
                    if local_apic.apic_id == apic::get_bsp_id() as u8 {
                        /*
                         * We do not want to start the BSP that is already running
                         * this code :D
                         */
                        continue;
                    }

                    if local_apic.flags & 1 != 1 {
                        // We cannot initialize disabled hardware :D
                        log::warn!("APIC {} is disabled by the hardware", local_apic.apic_id);
                    }

                    // Increase the CPU count.
                    CPU_COUNT.fetch_add(1, Ordering::SeqCst);

                    let ap_ready = unsafe {
                        let label = TRAMPOLINE_VIRTUAL.as_ptr::<u64>().offset(8);

                        label as *mut u64
                    };

                    let ap_cpu_id = unsafe { ap_ready.offset(1) };
                    let ap_page_table = unsafe { ap_ready.offset(2) };
                    let ap_stack_start = unsafe { ap_ready.offset(3) };
                    let ap_stack_end = unsafe { ap_ready.offset(4) };
                    let ap_code = unsafe { ap_ready.offset(5) };

                    let page_table = Cr3::read().0.start_address().as_u64();

                    let stack = malloc_align(4096 * 16, 4096);
                    let stack_end = unsafe { stack.offset(4096 * 16) } as u64;

                    unsafe {
                        intrinsics::atomic_store(ap_ready, 0x00);
                        intrinsics::atomic_store(ap_cpu_id, local_apic.apic_id as u64);
                        intrinsics::atomic_store(ap_page_table, page_table);
                        intrinsics::atomic_store(ap_stack_start, stack as u64);
                        intrinsics::atomic_store(ap_stack_end, stack_end);
                        intrinsics::atomic_store(ap_code, kernel_ap_startup as u64)
                    }

                    apic::mark_ap_ready(false);

                    let mut bsp = apic::get_local_apic();

                    // Send init IPI to the bsp.
                    unsafe {
                        let mut icr = 0x4500;

                        match bsp.apic_type() {
                            apic::ApicType::Xapic => icr |= (local_apic.apic_id as u64) << 56,
                            apic::ApicType::X2apic => icr |= (local_apic.apic_id as u64) << 32,
                            apic::ApicType::None => unreachable!(),
                        }

                        bsp.set_icr(icr);
                    }

                    // // Send start IPI to the bsp.
                    // unsafe {
                    //     let ap_segment = (TRAMPOLINE >> 12) & 0xFF;
                    //     let mut icr = 0x4600 | ap_segment as u64;

                    //     match bsp.apic_type() {
                    //         apic::ApicType::Xapic => icr |= (local_apic.apic_id as u64) << 56,
                    //         apic::ApicType::X2apic => icr |= (local_apic.apic_id as u64) << 32,
                    //         apic::ApicType::None => unreachable!(),
                    //     }

                    //     bsp.set_icr(icr);
                    // }

                    // unsafe {
                    //     // Wait for the AP to be ready.
                    //     while intrinsics::atomic_load(ap_ready) == 0 {
                    //         interrupts::pause();
                    //     }
                    // }

                    // // Wait for the trampoline to be ready.
                    // while !apic::ap_ready() {
                    //     interrupts::pause();
                    // }

                    log::info!("Loaded multicore");
                }

                MadtEntry::IoApic(io_apic) => apic::init_io_apic(io_apic),
                MadtEntry::IntSrcOverride(_) => {}
            }
        }

        /*
         * Now that we have initialized are APs. Its now safe to unmap the
         * AP trampoline and the trampoline region is marked as usable again.
         */
        let (_, toilet) = offset_table.unmap(trampoline_page).unwrap();
        toilet.flush();

        Ok(())
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
pub struct EntryHeader {
    entry_type: u8,
    length: u8,
}

#[repr(C, packed)]
struct MadtLocalApic {
    header: EntryHeader,
    processor_id: u8,
    apic_id: u8,
    flags: u32,
}

#[repr(C, packed)]
struct MadtIntSrcOverride {
    header: EntryHeader,
    bus: u8,
    irq: u8,
    global_system_interrupt: u32,
    flags: u16,
}

enum MadtEntry {
    LocalApic(&'static MadtLocalApic),
    IoApic(&'static IoApicHeader),
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
                let header = *(self.current as *const EntryHeader);

                self.current = self.current.offset(header.length as isize);

                let item = match header.entry_type {
                    0 => MadtEntry::LocalApic(&*(entry_pointer as *const MadtLocalApic)),
                    1 => MadtEntry::IoApic(&*(entry_pointer as *const IoApicHeader)),
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
