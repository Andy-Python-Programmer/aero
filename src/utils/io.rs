//! Wrapper functions for the hardware IO using respective assembly instructions.

/// Wrapper function to the `outb` assembly instruction used to do the
/// low level port output.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    asm!(
       "out dx, al",
       in("dx") port,
       in("al") value,
    );
}

/// Wrapper function to the `inb` assembly instruction used to do the
/// low level port input.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let ret: u8;

    asm!(
        "inb %dx, %al",
        in("dx") port,
        out("al") ret,
        options(att_syntax)
    );

    ret
}

/// This function is called after every `outb` instruction as on older machines
/// its necessary to give the PIC some time to react to commands as they might not
/// be processed quickly.
#[inline]
pub unsafe fn wait() {
    outb(0x80, 0)
}
