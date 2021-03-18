use crate::drivers::keyboard;
use crate::interrupts::end_pic1;
use crate::pit::PIT;
use crate::utils::io;

pub(crate) unsafe extern "x86-interrupt" fn pit() {
    PIT.tick();

    end_pic1();
}

pub(crate) unsafe extern "x86-interrupt" fn keyboard() {
    let scancode = io::inb(0x60);

    keyboard::handle(scancode);
    end_pic1();
}
