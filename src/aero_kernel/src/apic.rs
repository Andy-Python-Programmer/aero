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

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use core::{intrinsics, ptr};

use crate::mem::paging::VirtAddr;
use raw_cpuid::{CpuId, FeatureInfo};
use spin::Once;

use crate::utils::sync::{Mutex, MutexGuard};

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

/// Task Priority Register (TPR). Read/write. Bits 31:8 are reserved.
const XAPIC_TPR: u32 = 0x080;

static LOCAL_APIC: Once<Mutex<LocalApic>> = Once::new();
static BSP_APIC_ID: AtomicU64 = AtomicU64::new(0xFFFF_FFFF_FFFF_FFFF);

/// The count of all the active CPUs.
pub static CPU_COUNT: AtomicUsize = AtomicUsize::new(0);

static AP_READY: AtomicBool = AtomicBool::new(false);
static BSP_READY: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApicType {
    Xapic,
    X2apic,
    None,
}

impl ApicType {
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Return true if the current CPU supports X2 APIC.
    #[inline]
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
    /// Creates a new local APIC instance.
    ///
    /// ## Saftey
    /// The provided `address` points to a valid local APIC memory region and
    /// the `apic_type` is valid.
    fn new(address: VirtAddr, apic_type: ApicType) -> Self {
        Self { address, apic_type }
    }

    /// This function is responsible for initializing this instance of the local APIC.
    unsafe fn init(&mut self) {
        match self.apic_type {
            ApicType::Xapic => {
                // Clear the task priority register to enable all interrupts.
                self.write(XAPIC_TPR, 0x00);

                // Enable local APIC; set spurious interrupt vector.
                self.write(XAPIC_SVR, 0x100 | APIC_SPURIOUS_VECTOR);

                // Set up LVT (Local Vector Table) error.
                self.write(XAPIC_LVT_ERROR, 49);
            }

            ApicType::X2apic => {
                // Enable X2APIC (Bit 10)
                io::wrmsr(io::IA32_APIC_BASE, io::rdmsr(io::IA32_APIC_BASE) | 1 << 10);

                // Clear the task priority register to enable all interrupts.
                io::wrmsr(io::IA32_X2APIC_TPR, 0x00);

                // Set up LVT (Local Vector Table) error.
                io::wrmsr(io::IA32_X2APIC_LVT_ERROR, 49);

                // Enable local APIC; set spurious interrupt vector.
                io::wrmsr(io::IA32_X2APIC_SIVR, (0x100 | APIC_SPURIOUS_VECTOR) as _);
            }

            // Do nothing for the case of the None APIC type.
            ApicType::None => {}
        }
    }

    /// This function is responsible for sending IPI to the provided target logical
    /// processors by writing to the ICR register of this instance.
    ///
    /// ## Saftey
    /// The provided `cpu` must be a valid logical processor ID and the provided `vec` must be
    /// a valid interrupt vector.
    pub unsafe fn send_ipi(&mut self, cpu: usize, vec: u8) {
        match self.apic_type {
            ApicType::Xapic => {
                self.write(XAPIC_ICR1, (cpu as u32) << 24);
                self.write(XAPIC_ICR0, vec as _);

                // Make the ICR delivery status is clear, indicating that the
                // local APIC has completed sending the IPI. If set to 1 the
                // local APIC has not completed sending the IPI.
                while self.read(XAPIC_ICR0) & (1u32 << 12) > 0 {}
            }

            ApicType::X2apic => {
                io::wrmsr(io::IA32_X2APIC_ICR, vec as u64 | ((cpu as u64) << 32));

                // Make the ICR delivery status is clear, indicating that the
                // local APIC has completed sending the IPI. If set to 1 the
                // local APIC has not completed sending the IPI.
                while io::rdmsr(io::IA32_X2APIC_ICR) & (1u64 << 12) > 0 {}
            }

            // Do nothing for the case of the None APIC type.
            ApicType::None => {}
        }
    }

    /// At power up, system hardware assigns a unique APIC ID to each local APIC on the
    /// system bus. This function returns the unique APIC ID this instance.
    #[inline]
    fn bsp_id(&self) -> u32 {
        match self.apic_type {
            ApicType::Xapic => unsafe { self.read(0x20) },
            ApicType::X2apic => unsafe { io::rdmsr(io::IA32_X2APIC_APICID) as _ },
            ApicType::None => u32::MAX,
        }
    }

    /// The local APIC records errors detected during interrupt handling in the error status
    /// register (ESR). This function returns the value stored in the error status register
    /// of this instance.
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

    /// Writes to the EOI register to signal the end of an interrupt. This makes the local APIC
    /// to delete the interrupt from its ISR queue and send a message on the bus indicating that the
    /// interrupt handling has been completed.
    #[inline]
    pub unsafe fn eoi(&mut self) {
        match self.apic_type {
            ApicType::Xapic => self.write(XAPIC_EOI, 0x00),
            ApicType::X2apic => io::wrmsr(io::IA32_X2APIC_EOI, 0x00),
            ApicType::None => {}
        }
    }

    /// Returns the APIC type of this local APIC instance.
    #[inline]
    pub fn apic_type(&self) -> ApicType {
        self.apic_type
    }

    /// Sets the provided `value` to the ICR register of the instance.
    pub unsafe fn set_icr_xapic(&mut self, value_master: u32, value_slave: u32) {
        debug_assert!(self.apic_type == ApicType::Xapic); // Make sure we are dealing with XAPIC.

        self.write(XAPIC_ICR1, value_master);
        self.write(XAPIC_ICR0, value_slave);
    }

    /// Sets the provided `value` to the ICR register of this instance.
    #[inline]
    pub unsafe fn set_icr_x2apic(&mut self, value: u64) {
        debug_assert!(self.apic_type == ApicType::X2apic); // Make sure we are dealing with X2APIC.

        io::wrmsr(io::IA32_X2APIC_ICR, value);
    }

    /// Reads from the provided `register` as described by the MADT.
    #[inline]
    unsafe fn read(&self, register: u32) -> u32 {
        intrinsics::volatile_load((self.address + register as u64).as_u64() as *const u32)
    }

    /// Write to the provided `register` with the provided `data` as described by the MADT.
    #[inline]
    unsafe fn write(&mut self, register: u32, value: u32) {
        intrinsics::volatile_store((self.address + register as u64).as_u64() as *mut u32, value);
    }
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
#[inline]
pub fn get_bsp_id() -> u64 {
    BSP_APIC_ID.load(Ordering::SeqCst)
}

/// Return the number of active CPUs.
#[inline]
pub fn get_cpu_count() -> usize {
    CPU_COUNT.load(Ordering::Relaxed)
}

/// Returns true if the AP is ready.
#[inline]
pub fn ap_ready() -> bool {
    AP_READY.load(Ordering::SeqCst)
}

#[inline]
pub fn mark_ap_ready(value: bool) {
    AP_READY.store(value, Ordering::SeqCst);
}

#[inline]
pub fn is_bsp_ready() -> bool {
    BSP_READY.load(Ordering::SeqCst)
}

#[inline]
pub fn mark_bsp_ready(value: bool) {
    BSP_READY.store(value, Ordering::SeqCst);
}

/// Read from the `io_apic_id` I/O APIC as described by the MADT.
pub unsafe fn io_apic_read(io_apic_id: usize, register: u32) -> u32 {
    let io_apic = madt::IO_APICS.read()[io_apic_id];
    let addr = crate::PHYSICAL_MEMORY_OFFSET + io_apic.io_apic_address as usize;
    let ptr: *mut u32 = addr.as_mut_ptr();

    ptr::write_volatile(ptr, register);
    ptr::read(ptr.offset(4))
}

/// Write from the `io_apic_id` I/O APIC as described by the MADT.
pub unsafe fn io_apic_write(io_apic_id: usize, register: u32, data: u32) {
    let io_apic = madt::IO_APICS.read()[io_apic_id];
    let addr = crate::PHYSICAL_MEMORY_OFFSET + io_apic.io_apic_address as usize;
    let ptr: *mut u32 = addr.as_mut_ptr();

    ptr::write_volatile(ptr, register);
    ptr::write_volatile(ptr.offset(4), data)
}

/// Get the maximum number of redirects this I/O APIC can handle.
pub fn io_apic_get_max_redirect(io_apic_id: usize) -> u32 {
    unsafe { (io_apic_read(io_apic_id, 1) & 0xff0000) >> 16 }
}

/// Return the index of the I/O APIC that handles this redirect.
pub fn io_apic_from_redirect(gsi: u32) -> Option<usize> {
    let io_apics = madt::IO_APICS.read();

    for (i, entry) in io_apics.iter().enumerate() {
        let max_redirect = entry.global_system_interrupt_base + io_apic_get_max_redirect(i) > gsi;

        if entry.global_system_interrupt_base <= gsi || max_redirect {
            return Some(i);
        }
    }

    None
}

pub fn io_apic_set_redirect(vec: u8, gsi: u32, flags: u16, status: i32) {
    if let Some(io_apic) = io_apic_from_redirect(gsi) {
        let mut redirect = 0x00;

        // Active high(0) or low(1)
        if flags & 2 == 1 {
            redirect |= (1 << 13) as u8;
        }

        // Edge(0) or level(1) triggered
        if flags & 8 == 1 {
            redirect |= (1 << 15) as u8;
        }

        if status == 1 {
            // Set the mask bit
            redirect |= (1 << 16) as u8;
        }

        redirect |= vec;
        redirect |= (crate::arch::tls::get_cpuid() << 56) as u8; // Set the target APIC ID.

        let entry = madt::IO_APICS.read()[io_apic];
        let ioredtbl = (gsi - entry.global_system_interrupt_base) * 2 + 16;

        unsafe {
            io_apic_write(io_apic, ioredtbl + 0, redirect as _);
            io_apic_write(io_apic, ioredtbl + 1, (redirect as u64 >> 32) as _);
        }

        log::info!("registered redirect (vec={}, gsi={})", vec, gsi);
    } else {
        log::warn!("unable to register redirect (vec={}, gsi={})", vec, gsi);
    }
}

pub fn io_apic_setup_legacy_irq(irq: u8, status: i32) {
    // Redirect will handle weather IRQ is masked or not, we just need to
    // search the MADT ISOs for a corrosponsing IRQ.
    let isos_entries = madt::ISOS.read();

    for entry in isos_entries.iter() {
        if entry.irq == irq {
            io_apic_set_redirect(
                entry.irq + 0x20,
                entry.global_system_interrupt,
                entry.flags,
                status,
            );

            return;
        }
    }

    io_apic_set_redirect(irq + 0x20, irq as _, 0, status)
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
        local_apic.init();
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
