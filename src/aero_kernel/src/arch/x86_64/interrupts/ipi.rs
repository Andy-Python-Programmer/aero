use x86_64::instructions::tlb;

use super::interrupt_stack;
use crate::{apic, time};

interrupt_stack!(
    pub unsafe fn wakeup(stack: &mut InterruptStack) {
        apic::get_local_apic().eoi();
    }
);

interrupt_stack!(
    pub unsafe fn tlb(stack: &mut InterruptStack) {
        apic::get_local_apic().eoi();

        tlb::flush_all();
    }
);

interrupt_stack!(
    pub unsafe fn pit(stack: &mut InterruptStack) {
        apic::get_local_apic().eoi();

        time::PIT.tick();
    }
);
