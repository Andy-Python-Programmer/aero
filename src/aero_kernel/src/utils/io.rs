/*
 * Copyright 2021 The Aero Project Developers. See the COPYRIGHT
 * file at the top-level directory of this project.
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

//! Wrapper functions for the hardware IO using respective assembly instructions.

pub const IA32_EFER: u32 = 0xc0000080;

pub const IA32_FS_BASE: u32 = 0xC0000100;

/// System Call Target Address (R/W).
pub const IA32_STAR: u32 = 0xc0000081;

/// IA-32e Mode System Call Target Address (R/W).
pub const IA32_LSTAR: u32 = 0xc0000082;

/// System Call Flag Mask (R/W).
pub const IA32_FMASK: u32 = 0xc0000084;

/// APIC Location and Status (R/W).
pub const IA32_APIC_BASE: u32 = 0x1b;

/// Swap Target of BASE Address of GS (R/W).
pub const IA32_KERNEL_GSBASE: u32 = 0xc0000102;

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

/// Wrapper function to the `wrmsr` assembly instruction used
/// to write 64 bits to msr register.
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;

    asm!("wrmsr", in("ecx") msr, in("eax") low, in("edx") high, options(nomem));
}

/// Wrapper function to the `rdmsr` assembly instruction used
// to read 64 bits msr register.
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let (high, low): (u32, u32);

    asm!("rdmsr", out("eax") low, out("edx") high, in("ecx") msr, options(nomem));

    ((high as u64) << 32) | (low as u64)
}
