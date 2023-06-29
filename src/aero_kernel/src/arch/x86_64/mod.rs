// Copyright (C) 2021-2023 The Aero Project Developers.
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

pub mod cpu_local;

pub mod apic;
pub mod controlregs;
pub mod gdt;
pub mod interrupts;
pub mod io;
pub mod mem;
pub mod signals;
pub mod syscall;
pub mod task;
pub mod time;
pub mod tls;
pub mod user_copy;

use core::sync::atomic::Ordering;

use crate::acpi::aml;
use crate::{acpi, cmdline};

use crate::mem::paging;
use crate::mem::paging::VirtAddr;

use crate::{drivers, logger, rendy};

use raw_cpuid::CpuId;

use limine::*;
use spin::Once;

use self::interrupts::INTERRUPT_CONTROLLER;

static MEMMAP: MemmapRequest = MemmapRequest::new(0);
static SMP: SmpRequest = SmpRequest::new(0);
static KERNEL_FILE: KernelFileRequest = KernelFileRequest::new(0);
static MODULES: ModuleRequest = ModuleRequest::new(0);
static FRAMEBUFFER: FramebufferRequest = FramebufferRequest::new(0);
static RSDP: RsdpRequest = RsdpRequest::new(0);
static BOOT_TIME: BootTimeRequest = BootTimeRequest::new(0);
static STACK: StackSizeRequest = StackSizeRequest::new(0).stack_size(0x1000 * 32); // 16KiB of stack for both the BSP and the APs
static HHDM: HhdmRequest = HhdmRequest::new(0);

#[no_mangle]
extern "C" fn arch_aero_main() -> ! {
    unsafe {
        core::ptr::read_volatile(STACK.get_response().as_ptr().unwrap());
    }

    // SAFETY: We have exclusive access to the memory map.
    let memmap = MEMMAP
        .get_response()
        .get_mut()
        .expect("limine: invalid memmap response")
        .memmap_mut();

    unsafe {
        interrupts::disable_interrupts();
    }

    unsafe {
        crate::PHYSICAL_MEMORY_OFFSET = VirtAddr::new(HHDM.get_response().get().unwrap().offset);
    }

    let kernel_file_resp = KERNEL_FILE
        .get_response()
        .get()
        .expect("limine: invalid kernel file response");

    let kernel_file = kernel_file_resp
        .kernel_file
        .get()
        .expect("limine: invalid kernel file pointer");

    // Before we start the initialization process, we need to make sure
    // the unwind info is available; just in case if there is a kernel
    // panic, it will be able to unwind the stack.
    crate::unwind::UNWIND_INFO.call_once(|| {
        use crate::unwind::UnwindInfo;
        use xmas_elf::ElfFile;

        let start = kernel_file
            .base
            .as_ptr()
            .expect("limine: invalid kernel file base");

        // SAFETY: The bootloader will provide a valid pointer to the kernel file.
        let elf_slice = unsafe { core::slice::from_raw_parts(start, kernel_file.length as usize) };
        let elf = ElfFile::new(elf_slice).expect("limine: invalid kernel file");

        UnwindInfo::new(elf)
    });

    // Now that we have unwind info, we can initialize the COM ports. This
    // will be used to print panic messages/logs before the debug renderer is
    // initialized to the serial output (if available).
    drivers::uart::init();
    logger::init();

    // Initialize the CPU specific features.
    init_cpu();

    let modules = MODULES
        .get_response()
        .get()
        .expect("limine: invalid modules response")
        .modules();

    // Now, we need to parse the kernel command line so we can
    // setup the debug renderer.
    //
    // SAFETY: The `cmdline` is a valid, aligned, and NULL terminated string.
    let command_line = kernel_file
        .cmdline
        .to_str()
        .expect("limine: bad command line");

    let command_line = cmdline::parse(
        command_line.to_str().expect("cmdline: invalid utf8"),
        modules,
    );

    paging::init(memmap).unwrap();
    log::info!("loaded paging");

    crate::mem::alloc::init_heap();
    log::info!("loaded heap");

    // SMP initialization.
    let smp_response = SMP.get_response().get_mut().unwrap();
    let bsp_lapic_id = smp_response.bsp_lapic_id;

    for cpu in smp_response.cpus().iter_mut() {
        apic::CPU_COUNT.fetch_add(1, Ordering::SeqCst);

        if cpu.lapic_id == bsp_lapic_id {
            continue;
        }

        cpu.goto_address = x86_64_aero_ap_main;
    }

    gdt::init_boot();
    log::info!("loaded bootstrap GDT");

    paging::init_vm_frames();

    let framebuffer = FRAMEBUFFER
        .get_response()
        .get()
        .expect("limine: invalid framebuffer response")
        .framebuffers()
        .first()
        .expect("limine: no framebuffer found!");

    rendy::init(framebuffer, &command_line);
    logger::set_rendy_debug(true);

    interrupts::init();
    log::info!("loaded IDT");

    apic::init();
    log::info!("loaded APIC");

    let rsdp = VirtAddr::new(RSDP.get_response().get().unwrap().address.as_ptr().unwrap() as u64);

    acpi::init(rsdp);
    log::info!("loaded ACPI");

    tls::init();
    cpu_local::init(0);
    log::info!("loaded TLS");

    crate::unwind::set_panic_hook_ready(true);

    gdt::init();
    log::info!("loaded GDT");

    syscall::init();

    let boot_time = BOOT_TIME.get_response().get().unwrap();
    time::EPOCH.store(boot_time.boot_time as _, Ordering::SeqCst);

    // Architecture init is done. Now we can initialize and start the init
    // process in the non-architecture specific part of the kernel.
    crate::aero_main();
}

#[no_mangle]
extern "C" fn x86_64_aero_ap_main(boot_info: *const SmpInfo) -> ! {
    let boot_info = unsafe { &*boot_info };
    let ap_id = boot_info.processor_id as usize;

    log::debug!("booting CPU {}", ap_id);

    gdt::init_boot();
    log::info!("AP{}: loaded boot GDT", ap_id);

    tls::init();
    cpu_local::init(ap_id);
    log::info!("AP{}: loaded TLS", ap_id);

    gdt::init();
    log::info!("AP{}: loaded GDT", ap_id);

    syscall::init();

    // Wait for the BSP to be ready (after the BSP has initialized
    // the scheduler).
    while !apic::is_bsp_ready() {
        core::hint::spin_loop();
    }

    // Architecture init is done. Now move on to the non-architecture specific
    // initialization of the AP.
    crate::aero_ap_main(ap_id);
}

pub fn enable_acpi() {
    aml::get_subsystem().enable_acpi(INTERRUPT_CONTROLLER.method() as _);
}

fn enable_xsave() {
    use controlregs::XCr0Flags;

    // Enable XSAVE and x{get,set}bv
    let mut cr4 = controlregs::read_cr4();
    cr4.insert(controlregs::Cr4Flags::OSXSAVE);
    unsafe { controlregs::write_cr4(cr4) }

    let mut xcr0 = controlregs::read_xcr0();
    xcr0.insert(XCr0Flags::X87 | XCr0Flags::SSE);
    unsafe { controlregs::write_xcr0(xcr0) }
}

pub fn has_fsgsbase() -> bool {
    static HAS_FSGSBASE: Once<bool> = Once::new();

    *HAS_FSGSBASE.call_once(|| {
        CpuId::new()
            .get_extended_feature_info()
            .unwrap()
            .has_fsgsbase()
    })
}

pub fn init_cpu() {
    unsafe {
        // Enable the no-execute page protection feature.
        io::wrmsr(io::IA32_EFER, io::rdmsr(io::IA32_EFER) | 1 << 11);

        let features = CpuId::new().get_feature_info().unwrap();

        assert!(features.has_sse());

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

            if has_fsgsbase() {
                cr4.insert(controlregs::Cr4Flags::FSGSBASE);
            }

            controlregs::write_cr4(cr4);
        }

        assert!(features.has_xsave(), "init: xsave not supported!");
        enable_xsave();
    }
}
