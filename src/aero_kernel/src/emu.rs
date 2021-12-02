use crate::utils::io;

#[repr(u32)]
pub enum ExitStatus {
    Success = 0x10,
    Failure = 0x11,
}

pub fn exit_qemu(exit_status: ExitStatus) -> ! {
    // QEMU will execute `exit(((code << 1) | 1))`.
    unsafe {
        io::outl(0xf4, exit_status as u32);
    }

    // For the case that the QEMU exit attempt did not work, transition into an infinite loop.
    //
    // Calling `panic!()` here is unfeasible, since there is a good chance this function here is
    // the last expression in the `panic!()` handler itself. This prevents a possible infinite
    // loop.
    loop {
        unsafe {
            crate::arch::interrupts::halt();
        }
    }
}
