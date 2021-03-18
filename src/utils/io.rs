#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    asm!(
       "out dx, al",
       in("dx") port,
       in("al") value,
    );
}

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

#[inline]
pub unsafe fn wait() {
    outb(0x80, 0)
}
