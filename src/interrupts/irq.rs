use crate::drivers::keyboard;
use crate::interrupts::end_pic1;
use crate::utils::io;

pub(crate) extern "x86-interrupt" fn pit() {}

pub(crate) unsafe extern "x86-interrupt" fn keyboard() {
    let scancode = io::inb(0x60);

    keyboard::handle(scancode);
    end_pic1();
}
