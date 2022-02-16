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

use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::arch::{interrupts, tls};
use crate::mem::paging::{PhysAddr, VirtAddr};
use raw_cpuid::{CpuId, FeatureInfo};
use spin::Once;

use crate::utils::sync::{Mutex, MutexGuard};

use crate::acpi::madt;
use crate::utils::io;
use crate::{time, PHYSICAL_MEMORY_OFFSET};

const APIC_SPURIOUS_VECTOR: u32 = 0xFF;

/// LVT Error register. Read/write.
const XAPIC_LVT_ERROR: u32 = 0x370;

/// Error Status Register (ESR). Read/write.
const XAPIC_ESR: u32 = 0x280;

/// Spurious Interrupt Vector Register (SVR). Read/write.
const XAPIC_SVR: u32 = 0x0F0;

/// EOI register. Write-only.
const XAPIC_EOI: u32 = 0x0B0;

/// Interrupt Command Register (ICR). Read/write. (64-bit register)
const XAPIC_ICR: u32 = 0x300;

/// Task Priority Register (TPR). Read/write. Bits 31:8 are reserved.
const XAPIC_TPR: u32 = 0x080;

/// Local APIC ID register. Read-only. See Section 10.12.5.1 for initial values.
const XAPIC_ID: u32 = 0x020;

/// LVT Timer register. Read/write. See Figure 10-8 for reserved bits.
const XAPIC_LVT_TIMER: u32 = 0x320;

/// Initial Count register (for Timer). Read/write.
const XAPIC_TIMER_INIT_COUNT: u32 = 0x380;

/// Divide Configuration Register (DCR; for Timer). Read/write. See
/// Figure 10-10 for reserved bits.
const XAPIC_TIMER_DIV_CONF: u32 = 0x3E0;

/// Current Count register (for Timer). Read-only.
pub const XAPIC_TIMER_CURRENT_COUNT: u32 = 0x390;

const X2APIC_BASE_MSR: u32 = 0x800;

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
}

impl From<FeatureInfo> for ApicType {
    /// Processor support for XAPIC and X2APIC can be detected using
    /// `cpuid`. If the X2APIC bit is set the processor supports the X2APIC
    /// capability and can be placed into the X2APIC mode.
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
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    fn init(&mut self) {
        unsafe {
            if self.apic_type == ApicType::X2apic {
                // NOTE: We can place the local APIC in the X2APIC mode by setting the
                // X2APIC mode enable bit (bit 10) in the APIC base MSR.
                io::wrmsr(io::IA32_APIC_BASE, io::rdmsr(io::IA32_APIC_BASE) | 1 << 10);
            }

            // Clear the task priority register to enable all interrupts.
            self.write(XAPIC_TPR, 0x00);

            // Enable local APIC; set spurious interrupt vector.
            self.write(XAPIC_SVR, 0x100 | APIC_SPURIOUS_VECTOR);

            let lvt_err_vector = interrupts::allocate_vector();
            interrupts::register_handler(lvt_err_vector, interrupts::irq::lapic_error);

            // Set up LVT (Local Vector Table) error.
            self.write(XAPIC_LVT_ERROR, lvt_err_vector as u32);
        }
    }

    /// This function is responsible for sending IPI to the provided target logical
    /// processors by writing to the ICR register of this instance.
    ///
    /// ## Saftey
    /// The provided `cpu` must be a valid logical processor ID and the provided `vec` must be
    /// a valid interrupt vector.
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    pub unsafe fn send_ipi(&mut self, cpu: usize, vec: u8) {
        self.write_long(XAPIC_ICR, (cpu as u64) << 32 | vec as u64);

        // NOTE: Make the ICR delivery status is clear, indicating that the
        // local APIC has completed sending the IPI. If set to 1 the
        // local APIC has not completed sending the IPI.
        while self.read(XAPIC_ICR) & (1u32 << 12) > 0 {
            core::hint::spin_loop();
        }
    }

    /// At power up, system hardware assigns a unique APIC ID to each local APIC on the
    /// system bus. This function returns the unique APIC ID this instance.
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    fn bsp_id(&self) -> u32 {
        unsafe { self.read(XAPIC_ID) }
    }

    /// The local APIC records errors detected during interrupt handling in the error status
    /// register (ESR). This function returns the value stored in the error status register
    /// of this instance.
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    pub fn get_esr(&mut self) -> u32 {
        unsafe {
            self.write(XAPIC_ESR, 0);
            self.read(XAPIC_ESR)
        }
    }

    /// Writes to the EOI register to signal the end of an interrupt. This makes the local APIC
    /// to delete the interrupt from its ISR queue and send a message on the bus indicating that the
    /// interrupt handling has been completed.
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    pub fn eoi(&mut self) {
        unsafe {
            self.write(XAPIC_EOI, 0);
        }
    }

    /// Returns the APIC type of this instance.
    pub fn apic_type(&self) -> ApicType {
        self.apic_type
    }

    /// Stops the APIC timer.
    pub fn timer_stop(&mut self) {
        unsafe {
            self.write(XAPIC_TIMER_INIT_COUNT, 0);
            self.write(XAPIC_LVT_TIMER, 1 << 16);
        }
    }

    pub fn timer_oneshot(&mut self, vec: u8, us: usize) {
        self.timer_stop();

        let lapic_timer_frequency = tls::get_percpu().lapic_timer_frequency;
        let ticks = us * (lapic_timer_frequency / 1000000) as usize;

        unsafe {
            self.write(XAPIC_LVT_TIMER, vec as u32);
            self.write(XAPIC_TIMER_DIV_CONF, 0);
            self.write(XAPIC_TIMER_INIT_COUNT, ticks as u32);
        }
    }

    /// Calibrates the local APIC timer using the programmable interval timer.
    pub fn timer_calibrate(&mut self) {
        self.timer_stop();

        const SAMPLES: u32 = 0xfffff;

        unsafe {
            self.write(XAPIC_LVT_TIMER, (1 << 16) | 0xff); // vector 0xff, masked
            self.write(XAPIC_TIMER_DIV_CONF, 1);

            time::set_reload_value(0xffff);

            let initial_pit_tick = time::get_current_count();
            self.write(XAPIC_TIMER_INIT_COUNT, SAMPLES);

            while self.read(XAPIC_TIMER_CURRENT_COUNT) != 0 {}

            let final_pit_tick = time::get_current_count();
            let pit_ticks = initial_pit_tick - final_pit_tick;
            let timer_frequency = (SAMPLES / pit_ticks as u32) * time::PIT_DIVIDEND as u32;

            tls::get_percpu().lapic_timer_frequency = timer_frequency;
        }

        self.timer_stop();
    }

    /// Sets the provided `value` to the ICR register of the instance.
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    ///
    /// ## Safety
    /// The provided `value` must be a valid value for the ICR register.
    pub unsafe fn set_icr(&mut self, value: u64) {
        self.write_long(XAPIC_ICR, value);
    }

    /// Converts the provided APIC register (`register`) into its respective
    /// MSR for the X2APIC since, in X2APIC mode the `rdmsr` and `wrmsr`
    /// instructions are used to access the APIC registers.
    ///
    /// ## Safety
    /// The provided `register` must be a valid APIC register.
    unsafe fn register_to_x2apic_msr(&self, register: u32) -> u32 {
        X2APIC_BASE_MSR + (register >> 4)
    }

    /// Converts the provided APIC register (`register`) into its respective
    /// address for the XAPIC.
    ///
    /// ## Safety
    /// The provided `register` must be a valid APIC register.
    unsafe fn register_to_xapic_addr(&self, register: u32) -> VirtAddr {
        self.address + register as u64
    }

    /// Reads the provided APIC register (`register`) and returns its value.
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    ///
    /// ## Notes
    /// This function works for both XAPIC and X2APIC.
    ///
    /// ## Safety
    /// The provided `register` must be a valid APIC register.
    unsafe fn read(&self, register: u32) -> u32 {
        match self.apic_type {
            ApicType::X2apic => {
                let msr = self.register_to_x2apic_msr(register);
                io::rdmsr(msr) as _
            }

            ApicType::Xapic => {
                let addr = self.register_to_xapic_addr(register);
                addr.as_ptr::<u32>().read_volatile()
            }

            ApicType::None => unreachable!(),
        }
    }

    /// Writes the provided 32-bit value (`value`) to the provided
    /// APIC register (`register`).
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    ///
    /// ## Notes
    /// This function works for both XAPIC and X2APIC.
    ///
    /// ## Safety
    /// * The provided `register` must be a valid APIC register and the `value` must
    /// be a valid value for the provided APIC register.
    ///
    /// * If the `register` is 64-bit wide, then the [`Self::write_long`] function must
    /// be used instead.
    unsafe fn write(&mut self, register: u32, value: u32) {
        match self.apic_type {
            ApicType::X2apic => {
                let msr = self.register_to_x2apic_msr(register);
                io::wrmsr(msr, value as u64);
            }

            ApicType::Xapic => {
                let addr = self.register_to_xapic_addr(register);
                addr.as_mut_ptr::<u32>().write_volatile(value);
            }

            ApicType::None => unreachable!(),
        }
    }

    /// Writes the provided 64-bit value (`value`) to the provided
    /// APIC register (`register`).
    ///
    /// ## Panics
    /// * If the APIC type is set to [`ApicType::None`].
    ///
    /// ## Notes
    /// This function works for both XAPIC and X2APIC.
    ///
    /// ## Safety
    /// * The provided `register` must be a valid APIC register and the `value` must
    /// be a valid value for the provided APIC register.
    ///
    /// * If the `register` is 32-bit wide, then the [`Self::write`] function must
    /// be used instead.
    unsafe fn write_long(&mut self, register: u32, value: u64) {
        match self.apic_type {
            ApicType::X2apic => {
                let msr = self.register_to_x2apic_msr(register);
                io::wrmsr(msr, value);
            }

            ApicType::Xapic => {
                let addr_low = self.register_to_xapic_addr(register);
                let addr_high = self.register_to_xapic_addr(register + 0x10);

                addr_high.as_mut_ptr::<u32>().write_volatile(value as u32);
                addr_low
                    .as_mut_ptr::<u32>()
                    .write_volatile((value >> 32) as u32);
            }

            ApicType::None => unreachable!(),
        }
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

pub fn io_apic_setup_legacy_irq(irq: u8, vec: u8, status: i32) {
    // Redirect will handle weather IRQ is masked or not, we just need to
    // search the MADT ISOs for a corrosponsing IRQ.
    let isos_entries = madt::ISOS.read();

    for entry in isos_entries.iter() {
        if entry.irq == irq {
            io_apic_set_redirect(vec, entry.global_system_interrupt, entry.flags, status);
            return;
        }
    }

    io_apic_set_redirect(vec, irq as u32, 0, status)
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

    let apic_base = unsafe { io::rdmsr(io::IA32_APIC_BASE) };
    let address_phys = PhysAddr::new(apic_base & 0xFFFF0000);

    log::debug!("apic: detected APIC (addr={address_phys:?}, type={apic_type:?})");

    let address_virt = unsafe { PHYSICAL_MEMORY_OFFSET } + address_phys.as_u64();
    let mut local_apic = LocalApic::new(address_virt, apic_type);

    local_apic.init();

    let bsp_id = local_apic.bsp_id();

    BSP_APIC_ID.store(bsp_id as u64, Ordering::SeqCst);
    LOCAL_APIC.call_once(move || Mutex::new(local_apic));

    #[cfg(target_arch = "x86_64")]
    {
        use crate::arch::interrupts::INTERRUPT_CONTROLLER;

        // Now disable PIC as local APIC is initialized.
        //
        // SAFETY: Its safe to disable the PIC chip as now the local
        // APIC is initialized.
        INTERRUPT_CONTROLLER.switch_to_apic();
    }

    return apic_type;
}
