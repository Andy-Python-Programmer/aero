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
        "in al, dx",
        in("dx") port,
        out("al") ret,
    );

    ret
}

/// Wrapper function to the `outl` assembly instruction used to do the
/// low level port output.
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
    );
}

/// Wrapper function to the `inl` assembly instruction used to do the
/// low level port input.
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let ret: u32;

    asm!(
        "in eax, dx",
        in("dx") port,
        out("eax") ret,
    );

    ret
}

/// This function is called after every `outb` and `outl` instruction as on older machines
/// its necessary to give the PIC some time to react to commands as they might not
/// be processed quickly.
#[inline]
pub unsafe fn wait() {
    outb(0x80, 0)
}
