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

mod exceptions;
mod idt;
mod ipi;
mod irq;

use core::sync::atomic::{AtomicUsize, Ordering};

pub use idt::*;

use crate::apic;
use crate::utils::io;

use super::controlregs;

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;

const PIC2_DATA: u16 = 0xA1;
const PIC2_COMMAND: u16 = 0xA0;

const PIC_EOI: u8 = 0x20;

const ICW1_INIT: u8 = 0x10;
const ICW1_READ_ISR: u8 = 0x0B;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

lazy_static::lazy_static! {
    pub static ref PIC_CONTROLLER: PicController = PicController::new();
    pub static ref APIC_CONTROLLER: ApicController = ApicController;

    /// The global interrupt controller for x86 protected by a read-write lock.
    pub static ref INTERRUPT_CONTROLLER: InterruptController = InterruptController::new();
}

/// The interrupt controller interface. The task of an interrupt controller is to
/// end the interrupt, mask the interrupt, send ipi, etc...
#[repr(transparent)]
pub struct InterruptController {
    method: AtomicUsize,
}

impl InterruptController {
    /// Creates a new interrupt controller using the PIC chip by default.
    #[inline(always)]
    fn new() -> Self {
        Self {
            method: AtomicUsize::new(0),
        }
    }

    /// Send EOI, indicating the completion of an interrupt.
    pub fn eoi(&self) {
        match self.method.load(Ordering::Acquire) {
            0 => PIC_CONTROLLER.eoi(),
            1 => APIC_CONTROLLER.eoi(),

            _ => unreachable!(),
        }
    }

    /// Sets the interrupt controller to APIC.
    #[inline(always)]
    pub fn switch_to_apic(&self) {
        self.method.store(1, Ordering::Release);

        unsafe {
            PIC_CONTROLLER.disable();
        }
    }
}

/// APIC (Advanced Programmable Interrupt Controller) is an upgraded, advanced version
/// of the PIC chip. It is used for interrupt redirection, and sending interrupts between
/// processors.
///
/// ## Notes
/// * <https://wiki.osdev.org/APIC>
/// * <https://wiki.osdev.org/8259_PIC>
pub struct ApicController;

impl ApicController {
    /// Send EOI to the local APIC, indicating the completion of an interrupt.
    #[inline(always)]
    fn eoi(&self) {
        unsafe {
            apic::get_local_apic().eoi();
        }
    }
}

/// PIC (Programmable Interrupt Controller) manages hardware interrupts and sends
/// them to the appropriate system interrupt for the x86 architecture. Since APIC
/// has replaced PIC on modern systems, Aero disables PIC when APIC is avaliable.
///
/// ## Notes
/// * <https://wiki.osdev.org/8259_PIC>
/// * <https://wiki.osdev.org/APIC>
pub struct PicController;

impl PicController {
    /// Creates a new PIC controller. This function is responsible for remapping
    /// the PIC chip.
    fn new() -> Self {
        unsafe {
            let (a1, a2);

            a1 = io::inb(PIC1_DATA);
            io::wait();

            a2 = io::inb(PIC2_DATA);
            io::wait();

            io::outb(PIC1_COMMAND, ICW1_INIT | ICW1_ICW4);
            io::wait();
            io::outb(PIC2_COMMAND, ICW1_INIT | ICW1_ICW4);
            io::wait();

            io::outb(PIC1_DATA, 0x20);
            io::wait();
            io::outb(PIC2_DATA, 0x28);
            io::wait();

            io::outb(PIC1_DATA, 4);
            io::wait();
            io::outb(PIC2_DATA, 2);
            io::wait();

            io::outb(PIC1_DATA, ICW4_8086);
            io::wait();
            io::outb(PIC2_DATA, ICW4_8086);
            io::wait();

            io::outb(PIC1_DATA, a1);
            io::wait();
            io::outb(PIC2_DATA, a2);
            io::wait();

            io::outb(PIC1_DATA, 0b11111000);
            io::outb(PIC2_DATA, 0b11101111);
        }

        Self
    }

    /// Helper function to get the IRQ register. This function is responsible
    /// for sending the provided `command` PIC master to get the register values. PIC
    /// master represents IRQs 0-7, with 2 being the chain. PIC slave is chained, and
    /// represents IRQs 8-15.
    unsafe fn get_irq_register(&self, command: u8) -> u16 {
        io::outb(PIC2_COMMAND, command);
        io::wait();

        io::outb(PIC1_COMMAND, command);
        io::wait();

        let master_command = io::inb(PIC1_COMMAND) as u16;
        let slave_command = io::inb(PIC2_COMMAND) as u16;

        master_command << 8 | slave_command
    }

    /// Returns true if the PIC master chip is active.
    fn is_master_active(&self) -> bool {
        let isr = unsafe { self.get_irq_register(ICW1_READ_ISR) };

        (isr & 0xFF) > 0
    }

    /// Returns true if the PIC slave chip is active.
    fn is_slave_active(&self) -> bool {
        let isr = unsafe { self.get_irq_register(ICW1_READ_ISR) };

        (isr >> 8) > 0
    }

    /// Send EOI to the PIC chip, indicating the completion of an interrupt.
    fn eoi(&self) {
        if self.is_master_active() {
            unsafe { io::outb(PIC1_COMMAND, PIC_EOI) }
        } else if self.is_slave_active() {
            unsafe {
                io::outb(PIC2_COMMAND, PIC_EOI);
                io::outb(PIC1_COMMAND, PIC_EOI);
            }
        }
    }

    /// Disables the PIC interrupt controller.
    unsafe fn disable(&self) {
        log::debug!("Disabled PIC");

        io::outb(PIC1_DATA, 0xFF);
        io::wait();

        io::outb(PIC2_DATA, 0xFF);
        io::wait();
    }
}

/// Helper macro to generate an interrupt exception handler that expects an
/// error code.
pub macro interrupt_error_stack(fn $name:ident($stack:ident: &mut InterruptErrorStack) $code:block) {
    paste::item! {
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn [<__interrupt_ $name>](stack: *mut $crate::arch::interrupts::InterruptErrorStack) {
            #[inline(always)]
            #[allow(unused)] // Unused variable ($stack).
            fn inner($stack: &mut $crate::arch::interrupts::InterruptErrorStack) {
                $code
            }

            inner(&mut *stack);
        }

        $crate::utils::intel_fn!(
            pub extern "asm" fn $name() {
                $crate::utils::swapgs_iff_ring3_fast_errorcode!(),

                // Move rax into code's place and put code in last instead to be
                // compatible with interrupt stack.
                "xchg [rsp], rax\n",

                $crate::utils::push_scratch!(),
                $crate::utils::push_preserved!(),

                // Push the error code.
                "push rax\n",

                "call map_pti\n",

                // Call the inner interrupt handler implementation.
                "mov rdi, rsp\n",
                "call __interrupt_", stringify!($name), "\n",

                "call unmap_pti\n",

                // Pop the error code.
                "add rsp, 8\n",

                $crate::utils::pop_preserved!(),
                $crate::utils::pop_scratch!(),

                $crate::utils::swapgs_iff_ring3_fast!(),
                "iretq\n",
            }
        );
    }
}

/// Helper macro to generate an interrupt handler that takes the interrupt stack as
/// an argument.
pub macro interrupt_stack(pub unsafe fn $name:ident($stack:ident: &mut InterruptStack) $code:block) {
    paste::item! {
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn [<__interrupt_ $name>](stack: *mut $crate::arch::interrupts::InterruptStack) {
            #[inline(always)]
            #[allow(unused)] // Unused variable ($stack).
            unsafe fn inner($stack: &mut $crate::arch::interrupts::InterruptStack) {
                $code
            }

            inner(&mut *stack);
        }

        $crate::utils::intel_fn!(
            pub extern "asm" fn $name() {
                $crate::utils::swapgs_iff_ring3_fast!(),

                "push rax\n",

                $crate::utils::push_scratch!(),
                $crate::utils::push_preserved!(),

                "call map_pti\n",

                "mov rdi, rsp\n",
                "call __interrupt_", stringify!($name), "\n",

                "call unmap_pti\n",

                $crate::utils::pop_preserved!(),
                $crate::utils::pop_scratch!(),

                $crate::utils::swapgs_iff_ring3_fast!(),
                "iretq\n",
            }
        );
    }
}

/// Helper macro that generates an interrupt handler that does *not* require the
/// interrupt stack as an argument.
pub macro interrupt(pub unsafe fn $name:ident() $code:block) {
    paste::item! {
        #[no_mangle]
        #[doc(hidden)]
        unsafe extern "C" fn [<__interrupt_ $name>]() {
            $code
        }

        $crate::utils::intel_fn!(
            pub extern "asm" fn $name() {
                $crate::utils::swapgs_iff_ring3_fast!(),

                "push rax\n",

                $crate::utils::push_scratch!(),

                "call map_pti\n",

                "mov rdi, rsp\n",
                "call __interrupt_", stringify!($name), "\n",

                "call unmap_pti\n",

                $crate::utils::pop_scratch!(),

                $crate::utils::swapgs_iff_ring3_fast!(),
                "iretq\n",
            }
        );
    }
}

/// Wrapper function to the `hlt` assembly instruction used to halt
/// the CPU.
#[inline(always)]
pub unsafe fn halt() {
    asm!("hlt", options(nomem, nostack));
}

/// Wrapper function to the `cli` assembly instruction used to disable
/// interrupts.
#[inline(always)]
pub unsafe fn disable_interrupts() {
    asm!("cli", options(nomem, nostack));
}

/// Wrapper function to the `sti` assembly instruction used to enable
/// interrupts.
#[inline(always)]
pub unsafe fn enable_interrupts() {
    asm!("sti", options(nomem, nostack));
}

/// Returns true if interrupts are enabled.
#[inline(always)]
pub fn is_enabled() -> bool {
    controlregs::read_rflags().contains(controlregs::RFlags::INTERRUPT_FLAG)
}

/// Wrapper function to the `pause` assembly instruction used to pause
/// the cpu.
///
/// ## Saftey
/// Its safe to pause the CPU as the pause assembly instruction is similar
/// to the `nop` instruction and has no memory effects.
#[inline(always)]
pub fn pause() {
    unsafe {
        asm!("pause", options(nomem, nostack));
    }
}
