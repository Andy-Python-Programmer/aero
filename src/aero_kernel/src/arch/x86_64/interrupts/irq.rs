use crate::interrupt;
use crate::utils::io;
use crate::{
    apic,
    drivers::{keyboard, mouse},
};
use crate::{
    arch::interrupts::{end_pic1, end_pic2},
    time,
};

use super::InterruptStackFrame;

interrupt!(lapic_error, unsafe {
    log::error!("Local apic error");
    log::error!("ESR={:#0x}", apic::get_local_apic().get_esr());

    apic::get_local_apic().eoi();
});

interrupt!(pit, unsafe {
    time::PIT.tick();

    end_pic1();
});

interrupt!(keyboard, unsafe {
    let scancode = io::inb(0x60);

    keyboard::handle(scancode);
    end_pic1();
});

interrupt!(mouse, unsafe {
    let data = io::inb(0x60);

    mouse::handle(data);
    end_pic2();
});
