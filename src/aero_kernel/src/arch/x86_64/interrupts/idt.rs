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

//! The IDT is similar to the Global Descriptor Table in structure.
//!
//! **Notes**: <https://wiki.osdev.org/Interrupt_Descriptor_Table>

const IDT_ENTRIES: usize = 256;

pub const IPI_BASE: u8 = 0x40;

pub const IPI_ABORT: u8 = IPI_BASE + 0x00;
pub const IPI_RESCHEDULE: u8 = IPI_BASE + 0x01;

pub type InterruptHandlerFn = unsafe extern "C" fn();

static mut IDT: [IdtEntry; IDT_ENTRIES] = [IdtEntry::NULL; IDT_ENTRIES];

use core::mem::size_of;

use crate::arch::gdt::SegmentSelector;

bitflags::bitflags! {
    pub struct IDTFlags: u8 {
        const PRESENT = 1 << 7;
        const RING_0 = 0 << 5;
        const RING_1 = 1 << 5;
        const RING_2 = 2 << 5;
        const RING_3 = 3 << 5;
        const SS = 1 << 4;
        const INTERRUPT = 0xE;
        const TRAP = 0xF;
    }
}

#[repr(C, packed)]
struct IdtDescriptor {
    size: u16,
    offset: u64,
}

impl IdtDescriptor {
    /// Create a new IDT descriptor.
    #[inline]
    const fn new(size: u16, offset: u64) -> Self {
        Self { size, offset }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_middle: u16,
    offset_hi: u32,
    ignore: u32,
}

impl IdtEntry {
    /// IDT entry with all values defaulted to 0, ie `null`.
    const NULL: Self = Self {
        offset_low: 0x00,
        selector: 0x00,
        ist: 0x00,
        type_attr: 0x00,
        offset_middle: 0x00,
        offset_hi: 0x00,
        ignore: 0x00,
    };

    /// Set the IDT entry flags.
    fn set_flags(&mut self, flags: IDTFlags) {
        self.type_attr = flags.bits;
    }

    /// Set the IDT entry offset.
    fn set_offset(&mut self, selector: u16, base: usize) {
        self.selector = selector;
        self.offset_low = base as u16;
        self.offset_middle = (base >> 16) as u16;
        self.offset_hi = (base >> 32) as u32;
    }

    /// Set the handler function of the IDT entry.
    pub(crate) fn set_function(&mut self, handler: InterruptHandlerFn) {
        self.set_flags(IDTFlags::PRESENT | IDTFlags::RING_0 | IDTFlags::INTERRUPT);
        self.set_offset(8, handler as usize);
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ScratchRegisters {
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct PreservedRegisters {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct IretRegisters {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl IretRegisters {
    pub fn is_user(&self) -> bool {
        SegmentSelector::from_bits_truncate(self.cs as u16).contains(SegmentSelector::RPL_3)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct InterruptStack {
    pub preserved: PreservedRegisters,
    pub scratch: ScratchRegisters,
    pub iret: IretRegisters,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct InterruptErrorStack {
    pub code: u64,
    pub stack: InterruptStack,
}

/// Initialize the IDT.
pub fn init() {
    unsafe {
        IDT[0].set_function(super::exceptions::divide_by_zero);
        IDT[1].set_function(super::exceptions::debug);
        IDT[2].set_function(super::exceptions::non_maskable);
        IDT[3].set_function(super::exceptions::breakpoint);
        IDT[4].set_function(super::exceptions::overflow);
        IDT[5].set_function(super::exceptions::bound_range);
        IDT[6].set_function(super::exceptions::invalid_opcode);
        IDT[7].set_function(super::exceptions::device_not_available);
        IDT[8].set_function(super::exceptions::double_fault);

        // IDT[9] is reserved.

        IDT[10].set_function(super::exceptions::invalid_tss);
        IDT[11].set_function(super::exceptions::segment_not_present);
        IDT[12].set_function(super::exceptions::stack_segment);
        IDT[13].set_function(super::exceptions::protection);

        IDT[14].set_flags(IDTFlags::PRESENT | IDTFlags::RING_0 | IDTFlags::INTERRUPT);
        IDT[14].set_offset(8, super::exceptions::page_fault as usize);

        // IDT[15] is reserved.
        IDT[16].set_function(super::exceptions::fpu_fault);
        IDT[17].set_function(super::exceptions::alignment_check);
        IDT[18].set_function(super::exceptions::machine_check);
        IDT[19].set_function(super::exceptions::simd);
        IDT[20].set_function(super::exceptions::virtualization);

        // IDT[21..29] are reserved.
        IDT[30].set_function(super::exceptions::security);

        // Set up the IRQs.
        IDT[32].set_function(super::irq::pit_stack);
        IDT[33].set_function(super::irq::keyboard);
        IDT[44].set_function(super::irq::mouse);

        IDT[49].set_function(super::irq::lapic_error);

        IDT[IPI_ABORT as usize].set_function(super::ipi::abort);
        IDT[IPI_RESCHEDULE as usize].set_function(super::ipi::reschedule);

        let idt_descriptor = IdtDescriptor::new(
            ((IDT.len() * size_of::<IdtEntry>()) - 1) as u16,
            (&IDT as *const _) as u64,
        );

        load_idt(&idt_descriptor);

        /*
         * Since lazy statics are initialized on the their first dereference, we have to
         * manually initialize the static as the first dereference happen in an IRQ interrupt.
         * This means that the controller will never be initialized as an IRQ interrupt requires
         * the controller to be initialized.
         */
        lazy_static::initialize(&super::INTERRUPT_CONTROLLER);
        lazy_static::initialize(&super::PIC_CONTROLLER);
        lazy_static::initialize(&super::APIC_CONTROLLER);
    }
}

#[inline(always)]
unsafe fn load_idt(idt_descriptor: &IdtDescriptor) {
    asm!("lidt [{}]", in(reg) idt_descriptor, options(nostack));
}
