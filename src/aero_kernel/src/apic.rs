use core::intrinsics;

use core::sync::atomic::{AtomicU64, Ordering};

use spin::{Mutex, MutexGuard, Once};
use x86_64::VirtAddr;

use crate::utils::io;

static LOCAL_APIC: Once<Mutex<LocalApic>> = Once::new();
static BSP_APIC_ID: AtomicU64 = AtomicU64::new(0xFFFF_FFFF_FFFF_FFFF);

pub struct LocalApic(VirtAddr);

impl LocalApic {
    fn new(address: VirtAddr) -> Self {
        let mut this = Self(address);

        unsafe {
            this.init_ap();

            let apic_id = this.apic_id();
            BSP_APIC_ID.store(apic_id as u64, Ordering::SeqCst);
        }

        this
    }

    /// Initialize the application processor.
    unsafe fn init_ap(&mut self) {
        // Enable local APIC; set spurious interrupt vector.
        self.write(0xF0, self.read(0xF0) | 0x100);

        // Set up LVT (Local Vector Table) error interrupt.
        self.write(0x370, 0x31);
    }

    #[inline(always)]
    fn apic_id(&self) -> u32 {
        unsafe { self.read(0x20) }
    }

    /// Get the error code of the lapic by reading the error status register.
    pub unsafe fn get_esr(&mut self) -> u32 {
        self.write(0x280, 0);

        self.read(0x280)
    }

    #[inline(always)]
    pub unsafe fn eoi(&mut self) {
        self.write(0xB0, 0x00)
    }

    #[inline(always)]
    unsafe fn read(&self, register: u32) -> u32 {
        intrinsics::volatile_load((self.0 + register as usize).as_u64() as *const u32)
    }

    #[inline]
    unsafe fn write(&mut self, register: u32, value: u32) {
        intrinsics::volatile_store((self.0 + register as usize).as_u64() as *mut u32, value);

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
pub fn init(physical_memory_offset: VirtAddr) {
    unsafe {
        let address_phys = io::rdmsr(io::IA32_APIC_BASE) as usize & 0xFFFF_0000;
        let address_virt = physical_memory_offset + address_phys;

        log::debug!("Found apic at: {:#x}", address_phys);

        let local_apic = LocalApic::new(address_virt);
        LOCAL_APIC.call_once(move || Mutex::new(local_apic));
    }
}
