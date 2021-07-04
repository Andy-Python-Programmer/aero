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

use core::intrinsics;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::mem::paging::VirtAddr;
use raw_cpuid::{CpuId, FeatureInfo};
use spin::{Mutex, MutexGuard, Once};

use crate::acpi::madt;
use crate::utils::io;
use crate::PHYSICAL_MEMORY_OFFSET;

const APIC_SPURIOUS_VECTOR: u32 = 0xFF;

/// LVT Error register. Read/write.
const XAPIC_LVT_ERROR: u32 = 0x370;

/// Error Status Register (ESR). Read/write.
const XAPIC_ESR: u32 = 0x280;

/// Spurious Interrupt Vector Register (SVR). Read/write.
const XAPIC_SVR: u32 = 0x0F0;

/// EOI register. Write-only.
const XAPIC_EOI: u32 = 0x0B0;

/// Interrupt Command Register (ICR). Read/write.
const XAPIC_ICR0: u32 = 0x300;

/// Interrupt Command Register (ICR). Read/write.
const XAPIC_ICR1: u32 = 0x310;

static LOCAL_APIC: Once<Mutex<LocalApic>> = Once::new();
static BSP_APIC_ID: AtomicU64 = AtomicU64::new(0xFFFF_FFFF_FFFF_FFFF);

/// The count of all the active CPUs.
pub static CPU_COUNT: AtomicUsize = AtomicUsize::new(0);

static AP_READY: AtomicBool = AtomicBool::new(false);
static BSP_READY: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy)]
pub enum ApicType {
    Xapic,
    X2apic,
    None,
}

impl ApicType {
    #[inline(always)]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Return true if the current CPU supports X2 APIC.
    #[inline(always)]
    pub fn supports_x2_apic(&self) -> bool {
        matches!(self, Self::X2apic)
    }
}

impl From<FeatureInfo> for ApicType {
    fn from(feature_info: FeatureInfo) -> Self {
        if feature_info.has_x2apic() {
            Self::X2apic
        } else if feature_info.has_apic() {
            Self::Xapic
        } else {
            Self::None
        }
    }
}

pub struct LocalApic {
    address: VirtAddr,
    apic_type: ApicType,
}

impl LocalApic {
    fn new(address: VirtAddr, apic_type: ApicType) -> Self {
        Self { address, apic_type }
    }

    /// Initialize the application processor.
    unsafe fn init_cpu(&mut self) {
        match self.apic_type {
            ApicType::Xapic => {
                // Enable local APIC; set spurious interrupt vector.
                self.write(XAPIC_SVR, 0x100 | APIC_SPURIOUS_VECTOR);

                // Set up LVT (Local Vector Table) error.
                self.write(XAPIC_LVT_ERROR, 49);
            }

            ApicType::X2apic => {
                // Enable X2APIC (Bit 10)
                io::wrmsr(io::IA32_APIC_BASE, io::rdmsr(io::IA32_APIC_BASE) | 1 << 10);

                // Set up LVT (Local Vector Table) error.
                io::wrmsr(io::IA32_X2APIC_LVT_ERROR, 49);

                // Enable local APIC; set spurious interrupt vector.
                io::wrmsr(io::IA32_X2APIC_SIVR, (0x100 | APIC_SPURIOUS_VECTOR) as _);
            }

            ApicType::None => {}
        }
    }

    #[inline]
    fn bsp_id(&self) -> u32 {
        match self.apic_type {
            ApicType::Xapic => unsafe { self.read(0x20) },
            ApicType::X2apic => unsafe { io::rdmsr(io::IA32_X2APIC_APICID) as _ },
            ApicType::None => u32::MAX,
        }
    }

    /// Get the error code of the lapic by reading the error status register.
    pub unsafe fn get_esr(&mut self) -> u32 {
        match self.apic_type {
            ApicType::Xapic => {
                self.write(XAPIC_ESR, 0x00);
                self.read(XAPIC_ESR)
            }

            ApicType::X2apic => {
                io::wrmsr(io::IA32_X2APIC_ESR, 0x00);
                io::rdmsr(io::IA32_X2APIC_ESR) as _
            }

            ApicType::None => u32::MAX,
        }
    }

    #[inline]
    pub unsafe fn eoi(&mut self) {
        match self.apic_type {
            ApicType::Xapic => self.write(XAPIC_EOI, 0x00),
            ApicType::X2apic => io::wrmsr(io::IA32_X2APIC_EOI, 0x00),
            ApicType::None => {}
        }
    }

    #[inline]
    pub fn apic_type(&self) -> ApicType {
        self.apic_type
    }

    pub unsafe fn set_icr(&mut self, value: u64) {
        match self.apic_type {
            ApicType::Xapic => {
                while self.read(XAPIC_ICR0) & 1 << 12 == 1 << 12 {}

                self.write(XAPIC_ICR1, (value >> 32) as u32);
                self.write(XAPIC_ICR0, value as u32);

                while self.read(XAPIC_ICR0) & 1 << 12 == 1 << 12 {}
            }

            ApicType::X2apic => io::wrmsr(io::IA32_X2APIC_ICR, value),
            ApicType::None => {}
        }
    }

    #[inline]
    unsafe fn read(&self, register: u32) -> u32 {
        intrinsics::volatile_load((self.address + register as u64).as_u64() as *const u32)
    }

    #[inline]
    unsafe fn write(&mut self, register: u32, value: u32) {
        intrinsics::volatile_store((self.address + register as u64).as_u64() as *mut u32, value);
    }
}

#[repr(transparent)]
pub struct IoApic(VirtAddr);

impl IoApic {
    fn new(address_virt: VirtAddr) -> Self {
        Self(address_virt)
    }

    unsafe fn init(&mut self) {}
}

#[repr(C, packed)]
pub struct IoApicHeader {
    header: madt::EntryHeader,
    io_apic_id: u8,
    reserved: u8,
    io_apic_address: u32,
    global_system_interrupt_base: u32,
}

/// Get a mutable reference to the local apic.
pub fn get_local_apic() -> MutexGuard<'static, LocalApic> {
    LOCAL_APIC
        .get()
        .expect("Attempted to get the local apic before it was initialized")
        .lock()
}

/// Get the local BSP's id.
#[inline(always)]
pub fn get_bsp_id() -> u64 {
    BSP_APIC_ID.load(Ordering::SeqCst)
}

/// Return the number of active CPUs.
#[inline(always)]
pub fn get_cpu_count() -> usize {
    CPU_COUNT.load(Ordering::Relaxed)
}

/// Returns true if the AP is ready.
#[inline(always)]
pub fn ap_ready() -> bool {
    AP_READY.load(Ordering::SeqCst)
}

#[inline(always)]
pub fn mark_ap_ready(value: bool) {
    AP_READY.store(value, Ordering::SeqCst);
}

#[inline(always)]
pub fn is_bsp_ready() -> bool {
    BSP_READY.load(Ordering::SeqCst)
}

#[inline(always)]
pub fn mark_bsp_ready(value: bool) {
    BSP_READY.store(value, Ordering::SeqCst);
}

/// Initialize the IO apic. This function is called in the init function
/// of the [madt::Madt] acpi table.
pub fn init_io_apic(io_apic: &'static IoApicHeader) {
    let io_virtual = unsafe { PHYSICAL_MEMORY_OFFSET } + io_apic.io_apic_address as usize;

    let mut io_apic = IoApic::new(io_virtual);

    unsafe {
        io_apic.init();
    }
}

/// Initialize the local apic.
pub fn init() -> ApicType {
    let feature_info = CpuId::new()
        .get_feature_info()
        .expect("Failed to get CPU feature info");

    let apic_type = ApicType::from(feature_info);

    // Check if the current CPU is APIC compatible or not.
    if apic_type.is_none() {
        return apic_type;
    }

    let address_phys = unsafe { io::rdmsr(io::IA32_APIC_BASE) as usize & 0xFFFF_0000 };

    log::debug!("Found apic at: {:#x}", address_phys);

    let address_virt = unsafe { PHYSICAL_MEMORY_OFFSET } + address_phys;

    let mut local_apic = LocalApic::new(address_virt, apic_type);

    unsafe {
        local_apic.init_cpu();
    }

    // Now atomic store the BSP id.
    let bsp_id = local_apic.bsp_id();
    BSP_APIC_ID.store(bsp_id as u64, Ordering::SeqCst);

    LOCAL_APIC.call_once(move || Mutex::new(local_apic));

    #[cfg(target_arch = "x86_64")]
    {
        use crate::arch::interrupts::INTERRUPT_CONTROLLER;

        /*
         * Now disable PIC as local APIC is initialized.
         *
         * SAFTEY: Its safe to disable the PIC chip as now the local APIC is initialized.
         */
        INTERRUPT_CONTROLLER.switch_to_apic();
    }

    return apic_type;
}
