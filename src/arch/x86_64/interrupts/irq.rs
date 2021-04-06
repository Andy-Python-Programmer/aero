use crate::drivers::{keyboard, mouse};
use crate::utils::io;
use crate::{
    arch::interrupts::{end_pic1, end_pic2},
    time,
};

pub(crate) unsafe extern "x86-interrupt" fn pit() {
    time::PIT.tick();

    end_pic1();
}

pub(crate) unsafe extern "x86-interrupt" fn keyboard() {
    let scancode = io::inb(0x60);

    keyboard::handle(scancode);
    end_pic1();
}

pub(crate) unsafe extern "x86-interrupt" fn mouse() {
    let data = io::inb(0x60);

    mouse::handle(data);
    end_pic2();
}
