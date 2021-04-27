use core::intrinsics;

use core::sync::atomic::{AtomicU64, Ordering};

use raw_cpuid::{CpuId, FeatureInfo};
use spin::{Mutex, MutexGuard, Once};
use x86_64::VirtAddr;

use crate::arch::interrupts;
use crate::utils::io;

const APIC_SPURIOUS_VECTOR: u32 = 0xFF;

/// Task Priority Register (TPR). Read/write. Bits 31:8 are reserved.
const XAPIC_TPR: u32 = 0x080;

/// Spurious Interrupt Vector Register (SVR). Read/write.
const XAPIC_SVR: u32 = 0x0F0;

/// EOI register. Write-only.
const XAPIC_EOI: u32 = 0x0B0;

static LOCAL_APIC: Once<Mutex<LocalApic>> = Once::new();
static BSP_APIC_ID: AtomicU64 = AtomicU64::new(0xFFFF_FFFF_FFFF_FFFF);

#[derive(Debug, Clone, Copy)]
pub enum ApicType {
    Xapic,
    X2apic,
    None,
}

impl ApicType {
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
        let this = Self { address, apic_type };

        unsafe {
            let apic_id = this.bsp_id();
        }

        this
    }

    /// Initialize the application processor.
    unsafe fn init_cpu(&mut self) {
        // Clear task priority to enable interrupts.
        self.write(XAPIC_TPR, 0x00);

        // Enable local APIC; set spurious interrupt vector.
        self.write(XAPIC_SVR, 0x100 | APIC_SPURIOUS_VECTOR);
    }

    #[inline(always)]
    fn bsp_id(&self) -> u32 {
        unsafe { self.read(0x20) }
    }

    /// Get the error code of the lapic by reading the error status register.
    pub unsafe fn get_esr(&mut self) -> u32 {
        self.write(0x280, 0);

        self.read(0x280)
    }

    #[inline(always)]
    pub unsafe fn eoi(&mut self) {
        self.write(XAPIC_EOI, 0x00)
    }

    #[inline(always)]
    unsafe fn read(&self, register: u32) -> u32 {
        intrinsics::volatile_load((self.address + register as usize).as_u64() as *const u32)
    }

    #[inline]
    unsafe fn write(&mut self, register: u32, value: u32) {
        intrinsics::volatile_store(
            (self.address + register as usize).as_u64() as *mut u32,
            value,
        );

        self.read(0x20); // Wait for the write to finish.
    }
}

/// Get a mutable reference to the local apic.
pub fn get_local_apic() -> MutexGuard<'static, LocalApic> {
    LOCAL_APIC
        .get()
        .expect("Attempted to get the local apic before it was initialized")
        .lock()
}

/// Initialize the local apic.
pub fn init(physical_memory_offset: VirtAddr) -> ApicType {
    let feature_info = CpuId::new()
        .get_feature_info()
        .expect("Failed to get CPU feature info");

    let apic_type = ApicType::from(feature_info);

    // Check if the current CPU is APIC compatible or not.
    if matches!(apic_type, ApicType::None) {
        return ApicType::None;
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
