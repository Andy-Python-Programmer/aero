use crate::drivers::{keyboard, mouse};
use crate::interrupt;
use crate::utils::io;
use crate::{
    arch::interrupts::{end_pic1, end_pic2},
    time,
};

use super::InterruptStackFrame;

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
