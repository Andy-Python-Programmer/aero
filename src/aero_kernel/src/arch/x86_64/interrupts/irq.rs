use super::interrupt;
use crate::utils::io;
use crate::{
    apic,
    drivers::{keyboard, mouse},
};
use crate::{
    arch::interrupts::{end_pic1, end_pic2},
    time,
};

interrupt!(
    pub unsafe fn lapic_error(stack: InterruptStack) {
        log::error!("Local apic error");
        log::error!("ESR={:#0x}", apic::get_local_apic().get_esr());

        apic::get_local_apic().eoi();

        loop {}
    }
);

interrupt!(
    pub unsafe fn pit(stack: InterruptStack) {
        time::PIT.tick();

        end_pic1();
    }
);

interrupt!(
    pub unsafe fn keyboard(stack: InterruptStack) {
        let scancode = io::inb(0x60);

        keyboard::handle(scancode);
        end_pic1();
    }
);

interrupt!(
    pub unsafe fn mouse(stack: InterruptStack) {
        let data = io::inb(0x60);

        mouse::handle(data);
        end_pic2();
    }
);
