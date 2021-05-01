use core::intrinsics;

use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use raw_cpuid::{CpuId, FeatureInfo};
use spin::{Mutex, MutexGuard, Once};
use x86_64::VirtAddr;

use crate::arch::interrupts;
use crate::utils::io;

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
        // Enable local APIC; set spurious interrupt vector.
        self.write(XAPIC_SVR, 0x100 | APIC_SPURIOUS_VECTOR);

        // Set up LVT (Local Vector Table) error.
        self.write(XAPIC_LVT_ERROR, 49);
    }

    #[inline(always)]
    fn bsp_id(&self) -> u32 {
        unsafe { self.read(0x20) }
    }

    /// Get the error code of the lapic by reading the error status register.
    pub unsafe fn get_esr(&mut self) -> u32 {
        self.write(XAPIC_ESR, 0x00);

        self.read(XAPIC_ESR)
    }

    #[inline(always)]
    pub unsafe fn eoi(&mut self) {
        self.write(XAPIC_EOI, 0x00)
    }

    #[inline(always)]
    pub fn apic_type(&self) -> ApicType {
        self.apic_type
    }

    pub unsafe fn set_icr(&mut self, value: u64) {
        while self.read(XAPIC_ICR0) & 1 << 12 == 1 << 12 {}

        self.write(XAPIC_ICR1, (value >> 32) as u32);
        self.write(XAPIC_ICR0, value as u32);

        while self.read(XAPIC_ICR0) & 1 << 12 == 1 << 12 {}
    }

    #[inline(always)]
    unsafe fn read(&self, register: u32) -> u32 {
        intrinsics::volatile_load((self.address + register as usize).as_u64() as *const u32)
    }

    #[inline(always)]
    unsafe fn write(&mut self, register: u32, value: u32) {
        intrinsics::volatile_store(
            (self.address + register as usize).as_u64() as *mut u32,
            value,
        );
    }
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

/// Initialize the local apic.
pub fn init(physical_memory_offset: VirtAddr) -> ApicType {
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

    let address_virt = physical_memory_offset + address_phys;

    let mut local_apic = LocalApic::new(address_virt, apic_type);

    unsafe {
        local_apic.init_cpu();
    }

    // Now atomic store the BSP id.
    let bsp_id = local_apic.bsp_id();
    BSP_APIC_ID.store(bsp_id as u64, Ordering::SeqCst);

    LOCAL_APIC.call_once(move || Mutex::new(local_apic));

    // Now disable PIC as local APIC is initialized.
    //
    // SAFTEY: Its safe to disable the PIC chip as now the local APIC is initialized.
    unsafe { interrupts::disable_pic() };
    log::info!("Disabled PIC");

    return apic_type;
}
