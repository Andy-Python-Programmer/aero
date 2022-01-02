/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

//! Wrapper functions for the hardware IO using respective assembly instructions.

pub const IA32_EFER: u32 = 0xc0000080;

pub const IA32_FS_BASE: u32 = 0xC0000100;
pub const IA32_GS_BASE: u32 = 0xc0000101;

/// System Call Target Address (R/W).
pub const IA32_STAR: u32 = 0xc0000081;

/// IA-32e Mode System Call Target Address (R/W).
pub const IA32_LSTAR: u32 = 0xc0000082;

/// System Call Flag Mask (R/W).
pub const IA32_FMASK: u32 = 0xc0000084;

/// APIC Location and Status (R/W).
pub const IA32_APIC_BASE: u32 = 0x1b;

/// x2APIC Spurious Interrupt Vector register (R/W)
pub const IA32_X2APIC_SIVR: u32 = 0x80f;

/// x2APIC ID register (R/O) See X2APIC Specification.
pub const IA32_X2APIC_APICID: u32 = 0x802;

/// Error Status Register.
pub const IA32_X2APIC_ESR: u32 = 0x828;

/// x2APIC Interrupt Command register (R/W)
pub const IA32_X2APIC_ICR: u32 = 0x830;

/// x2APIC End of Interrupt.
pub const IA32_X2APIC_EOI: u32 = 0x80b;

pub const IA32_X2APIC_LVT_ERROR: u32 = 0x837;

/// x2APIC Task Priority register (R/W)
pub const IA32_X2APIC_TPR: u32 = 0x808;

/// Wrapper function to the `outb` assembly instruction used to do the
/// 8-bit low level port output.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    asm!(
       "out dx, al",
       in("dx") port,
       in("al") value,
       options(preserves_flags, nomem, nostack)
    );
}

/// Wrapper function to the `inb` assembly instruction used to do the
/// 8-bit low level port input.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let ret: u8;

    asm!(
        "in al, dx",
        in("dx") port,
        out("al") ret,
        options(preserves_flags, nomem, nostack)
    );

    ret
}

/// Wrapper function to the `outw` assembly instruction used to do the
/// 16-bit low level port output.
#[inline]
pub unsafe fn outw(port: u16, value: u16) {
    asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
        options(preserves_flags, nomem, nostack)
    );
}

/// Wrapper function to the `outl` assembly instruction used to do the
/// low level port output.
#[inline]
pub unsafe fn outl(port: u16, value: u32) {
    asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") value,
        options(preserves_flags, nomem, nostack)
    );
}

/// Wrapper function to the `inl` assembly instruction used to do the
/// 32-bit low level port input.
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let ret: u32;

    asm!(
        "in eax, dx",
        in("dx") port,
        out("eax") ret,
        options(preserves_flags, nomem, nostack)
    );

    ret
}

/// Wrapper function to the `inw` assembly instruction used to do the
/// 16-bit low level port input.
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let ret: u16;

    asm!(
        "in eax, dx",
        out("eax") ret,
        in("dx") port,
        options(preserves_flags, nomem, nostack)
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

#[inline]
pub fn delay(cycles: usize) {
    unsafe {
        for _ in 0..cycles {
            inb(0x80);
        }
    }
}

pub trait InOut {
    unsafe fn port_in(port: u16) -> Self;
    unsafe fn port_out(port: u16, value: Self);
}

impl InOut for u8 {
    unsafe fn port_in(port: u16) -> u8 {
        inb(port)
    }

    unsafe fn port_out(port: u16, value: u8) {
        outb(port, value);
    }
}

impl InOut for u16 {
    unsafe fn port_in(port: u16) -> u16 {
        inw(port)
    }

    unsafe fn port_out(port: u16, value: u16) {
        outw(port, value);
    }
}

impl InOut for u32 {
    unsafe fn port_in(port: u16) -> u32 {
        inl(port)
    }

    unsafe fn port_out(port: u16, value: u32) {
        outl(port, value);
    }
}

// based :^)

#[derive(Copy, Clone)]
pub struct BasedPort {
    base: u16,
}

impl BasedPort {
    pub fn new(base: u16) -> BasedPort {
        BasedPort { base }
    }

    pub fn read_offset<V: InOut>(&self, offset: u16) -> V {
        unsafe { V::port_in(self.base + offset) }
    }

    pub fn write_offset<V: InOut>(&mut self, offset: u16, value: V) {
        unsafe { V::port_out(self.base + offset, value) }
    }
}
