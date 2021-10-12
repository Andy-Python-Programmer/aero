/*
 * Copyright (C) 2021 The Aero Project Developers.
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

pub mod controlregs;
pub mod gdt;
pub mod interrupts;
pub mod task;

use crate::acpi;
use crate::apic;
use crate::cmdline;
use crate::mem;
use crate::mem::alloc;
use crate::mem::paging;

use crate::mem::paging::{PhysAddr, VirtAddr};

use crate::drivers;
use crate::logger;
use crate::rendy;
use crate::tls;
use crate::utils::io;

use raw_cpuid::CpuId;
use stivale_boot::v2::*;
use xmas_elf::sections::ShType;

#[repr(C, align(4096))]
struct P2Align12<T>(T);

const STACK_SIZE: usize = 4096 * 16;

/// We need to tell the stivale bootloader where we want our stack to be.
/// We are going to allocate our stack as an uninitialised array in .bss.
static STACK: P2Align12<[u8; STACK_SIZE]> = P2Align12([0; STACK_SIZE]);

/// We are now going to define a framebuffer header tag. This tag tells the bootloader that
/// we want a graphical framebuffer instead of a CGA-compatible text mode. Omitting this tag will
/// make the bootloader default to text mode, if available.
static FRAMEBUFFER_TAG: StivaleFramebufferHeaderTag = StivaleFramebufferHeaderTag::new()
    .framebuffer_bpp(24)
    .next((&PAGING_TAG as *const Stivale5LevelPagingHeaderTag).cast());

/// We are now going to define a level 5 paging header tag. This tag tells the bootloader to
/// enable the LEVEL_5_PAGING bit in the Cr4 register. This is not possible to implement in the kernel
/// as we can only enable it in protected mode.
static PAGING_TAG: Stivale5LevelPagingHeaderTag = Stivale5LevelPagingHeaderTag::new();

/// The stivale2 specification says we need to define a "header structure".
/// This structure needs to reside in the .stivale2hdr ELF section in order
/// for the bootloader to find it. We use the #[linker_section] and #[used] macros to
/// tell the compiler to put the following structure in said section.
#[link_section = ".stivale2hdr"]
#[no_mangle]
#[used]
static STIVALE_HDR: StivaleHeader = StivaleHeader::new()
    .stack(&STACK.0[STACK_SIZE - 4096] as *const u8)
    .tags((&FRAMEBUFFER_TAG as *const StivaleFramebufferHeaderTag).cast());

#[no_mangle]
extern "C" fn x86_64_aero_main(boot_info: &'static StivaleStruct) -> ! {
    let mmap_tag = boot_info
        .memory_map()
        .expect("stivale2: aero requires the bootloader to provide a non-null memory map tag");

    let rsdp_tag = boot_info
        .rsdp()
        .expect("stivale2: aero requires the bootloader to provided a non-null rsdp tag");

    let framebuffer_tag = boot_info
        .framebuffer()
        .expect("stivale2: aero requires the bootloader to provide a non-null framebuffer tag");

    let kernel_info = boot_info
        .kernel_file_v2()
        .expect("stivale2: aero requires the bootloader to provode a non-null kernel info V2 tag");

    let rsdp_address = PhysAddr::new(rsdp_tag.rsdp);

    // NOTE: STACK_SIZE - 1 points to the last u8 in the array, i.e. it is
    // guaranteed to be at an address with its least significant bit being a 1
    // and it never has an alignment greater than 1. STACK_SIZE - 4096 points
    // to the last u8 in STACK, that is aligned to 4096.
    let stack_top_addr = VirtAddr::new((&STACK.0[STACK_SIZE - 4096] as *const u8) as _);

    unsafe {
        interrupts::disable_interrupts();
    }

    if paging::level_5_paging_enabled() {
        unsafe {
            crate::PHYSICAL_MEMORY_OFFSET = VirtAddr::new(0xff00000000000000);
        }
    } else {
        unsafe {
            crate::PHYSICAL_MEMORY_OFFSET = VirtAddr::new(0xffff800000000000);
        }
    }

    crate::UNWIND_INFO.call_once(move || unsafe {
        let addr = (kernel_info as *const StivaleKernelFileV2Tag) as u64;
        let new_addr = crate::PHYSICAL_MEMORY_OFFSET + addr;

        &*new_addr.as_mut_ptr::<StivaleKernelFileV2Tag>()
    });

    // Initialize the CPU specific features.
    init_cpu();

    // We initialize the COM ports before doing anything else.
    //
    // This will help printing panics and logs before or when the debug renderer
    // is initialized and if serial output is avaliable.
    drivers::uart_16550::init();
    logger::init();

    rendy::init(framebuffer_tag);

    let (kernel_base, kernel_end) = {
        let kernel_slice: &[u8] = unsafe {
            core::slice::from_raw_parts(
                (crate::PHYSICAL_MEMORY_OFFSET + kernel_info.kernel_start).as_ptr(),
                kernel_info.kernel_size as usize,
            )
        };

        let kernel_elf =
            xmas_elf::ElfFile::new(kernel_slice).expect("stivale2: invalid kernel slice");

        let mut kernel_start = None;
        let mut kernel_end = 0x00;

        for section in kernel_elf.section_iter() {
            let is_null = section
                .get_type()
                .map_or(true, |section| section == ShType::Null);

            if section.address() != 0x00 && !is_null {
                if kernel_start.is_none() {
                    kernel_start = Some(section.address());
                }

                kernel_end = section.address() + section.size();
            }
        }

        (
            // NOTE: Higher half offset starts at address 0xffffffff80000000 and the aero kernel
            // is placed 2MiB above the higher half offset (2MiB above 0x00 in physical memory).
            //
            // So, the kernel base address will be 2MiB. However, we do currently dynamically figure
            // the offset out since when we add support for KALSR its more easier to implement.
            PhysAddr::new(kernel_start.expect("mem: kernel top not found") - 0xFFFFFFFF80000000),
            PhysAddr::new(kernel_end - 0xFFFFFFFF80000000),
        )
    };

    // Parse the kernel command line.
    let command_line: &'static _ = boot_info.command_line().map_or("", |cmd| unsafe {
        // SAFETY: The bootloader has provided a pointer that points to a valid C
        // string with a NULL terminator of size less than `usize::MAX`, whose content
        // remain valid and has a static lifetime.
        mem::c_str_as_str(cmd.command_line as *const u8)
    });

    let command_line = cmdline::parse(command_line);
    logger::set_rendy_debug(command_line.rendy_debug);

    gdt::init_boot();
    log::info!("loaded bootstrap GDT");

    let mut offset_table = paging::init(mmap_tag, kernel_base, kernel_end).unwrap();
    log::info!("loaded paging");

    alloc::init_heap(&mut offset_table).expect("failed to initialize the kernel heap");
    log::info!("loaded heap");

    interrupts::init();
    log::info!("loaded IDT");

    let apic_type = apic::init();
    log::info!(
        "Loaded local apic (x2apic={})",
        apic_type.supports_x2_apic()
    );

    acpi::init(rsdp_address).unwrap();
    log::info!("Loaded ACPI");

    tls::init();
    log::info!("loaded TLS");

    gdt::init(stack_top_addr);
    log::info!("loaded GDT");

    // Initialize the non-arch specific parts of the kernel.
    crate::aero_main();
}

pub fn init_cpu() {
    unsafe {
        // Enable the no-execute page protection feature.
        io::wrmsr(io::IA32_EFER, io::rdmsr(io::IA32_EFER) | 1 << 11);

        // Check if SSE is supported. SSE support is a requirement for running Aero.
        let has_sse = CpuId::new()
            .get_feature_info()
            .map_or(false, |i| i.has_sse());

        assert!(has_sse);

        {
            let mut cr0 = controlregs::read_cr0();

            cr0.remove(controlregs::Cr0Flags::EMULATE_COPROCESSOR);
            cr0.insert(controlregs::Cr0Flags::MONITOR_COPROCESSOR);

            controlregs::write_cr0(cr0);
        }

        {
            let mut cr4 = controlregs::read_cr4();

            cr4.insert(controlregs::Cr4Flags::OSFXSR);
            cr4.insert(controlregs::Cr4Flags::OSXMMEXCPT_ENABLE);

            controlregs::write_cr4(cr4);
        }
    }
}
