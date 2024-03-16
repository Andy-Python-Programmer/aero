// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

//! The IDT is similar to the Global Descriptor Table in structure.
//!
//! **Notes**: <https://wiki.osdev.org/Interrupt_Descriptor_Table>

const IDT_ENTRIES: usize = 256;

pub(super) static mut IDT: [IdtEntry; IDT_ENTRIES] = [IdtEntry::EMPTY; IDT_ENTRIES];

use core::mem::size_of;
use core::ptr::addr_of;

use bit_field::BitField;

use crate::arch::gdt::{GdtEntryIndex, PrivilegeLevel, SegmentSelector};
use crate::utils::sync::Mutex;

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

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct EntryOptions {
    cs: SegmentSelector,
    bits: u16,
}

impl EntryOptions {
    #[inline]
    const fn default() -> Self {
        Self {
            cs: SegmentSelector::empty(),
            // bits 11:8 specify the gate-descriptor type, default to 64-bit interrupt gate (0xe).
            bits: 0b1110_0000_0000,
        }
    }

    #[inline]
    fn set_privilege_level(&mut self, dpl: PrivilegeLevel) {
        self.bits.set_bits(13..15, dpl as u16);
    }

    #[inline]
    fn set_code_segment(&mut self, cs: SegmentSelector) {
        self.cs = cs;
    }

    #[inline]
    fn set_present(&mut self, present: bool) {
        self.bits.set_bit(15, present);
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(super) struct IdtEntry {
    ptr_low: u16,
    options: EntryOptions,
    ptr_middle: u16,
    ptr_high: u32,
    ignore: u32,
}

impl IdtEntry {
    const EMPTY: Self = Self {
        ptr_low: 0,
        options: EntryOptions::default(),
        ptr_middle: 0,
        ptr_high: 0,
        ignore: 0,
    };

    pub(crate) fn set_function(&mut self, ptr: *const u8) {
        self.options.set_privilege_level(PrivilegeLevel::Ring0);
        self.options.set_code_segment(SegmentSelector::new(
            GdtEntryIndex::KERNEL_CODE,
            PrivilegeLevel::Ring0,
        ));

        let addr = ptr.addr();

        self.ptr_low = addr as u16;
        self.ptr_middle = (addr >> 16) as u16;
        self.ptr_high = (addr >> 32) as u32;

        self.options.set_present(true);
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
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

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct PreservedRegisters {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct IretRegisters {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl IretRegisters {
    pub fn is_user(&self) -> bool {
        let selector = SegmentSelector::from_bits(self.cs as u16);
        selector.privilege_level().is_user()
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct InterruptStack {
    pub preserved: PreservedRegisters,
    pub scratch: ScratchRegisters,
    pub iret: IretRegisters,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct InterruptErrorStack {
    pub code: u64,
    pub stack: InterruptStack,
}

#[derive(Copy, Clone)]
pub(super) enum IrqHandler {
    ErrorHandler(fn(&mut InterruptErrorStack)),
    Handler(fn(&mut InterruptStack)),

    None,
}

pub(super) static INTERRUPT_HANDLERS: Mutex<[IrqHandler; IDT_ENTRIES]> =
    Mutex::new([IrqHandler::None; IDT_ENTRIES]);

/// Initialize the IDT.
pub fn init() {
    use super::exceptions;

    extern "C" {
        // defined in `handlers.asm`
        static interrupt_handlers: [*const u8; IDT_ENTRIES];
    }

    unsafe {
        for (index, &handler) in interrupt_handlers.iter().enumerate() {
            // skip handler insertion if handler is null.
            if handler.is_null() {
                continue;
            }

            IDT[index].set_function(handler);
        }
    }

    INTERRUPT_HANDLERS.lock()[0] = IrqHandler::ErrorHandler(exceptions::divide_by_zero);
    INTERRUPT_HANDLERS.lock()[1] = IrqHandler::ErrorHandler(exceptions::debug);
    INTERRUPT_HANDLERS.lock()[2] = IrqHandler::ErrorHandler(exceptions::non_maskable);
    INTERRUPT_HANDLERS.lock()[3] = IrqHandler::ErrorHandler(exceptions::breakpoint);
    INTERRUPT_HANDLERS.lock()[4] = IrqHandler::ErrorHandler(exceptions::overflow);
    INTERRUPT_HANDLERS.lock()[5] = IrqHandler::ErrorHandler(exceptions::bound_range);
    INTERRUPT_HANDLERS.lock()[6] = IrqHandler::ErrorHandler(exceptions::invalid_opcode);
    INTERRUPT_HANDLERS.lock()[7] = IrqHandler::ErrorHandler(exceptions::device_not_available);
    INTERRUPT_HANDLERS.lock()[8] = IrqHandler::ErrorHandler(exceptions::double_fault);

    // INTERRUPT_HANDLERS[9] is reserved.
    INTERRUPT_HANDLERS.lock()[10] = IrqHandler::ErrorHandler(exceptions::invalid_tss);
    INTERRUPT_HANDLERS.lock()[11] = IrqHandler::ErrorHandler(exceptions::segment_not_present);
    INTERRUPT_HANDLERS.lock()[12] = IrqHandler::ErrorHandler(exceptions::stack_segment);
    INTERRUPT_HANDLERS.lock()[13] = IrqHandler::ErrorHandler(exceptions::protection);
    INTERRUPT_HANDLERS.lock()[14] = IrqHandler::ErrorHandler(exceptions::page_fault);

    // INTERRUPT_HANDLERS[15] is reserved.
    INTERRUPT_HANDLERS.lock()[16] = IrqHandler::ErrorHandler(exceptions::fpu_fault);
    INTERRUPT_HANDLERS.lock()[17] = IrqHandler::ErrorHandler(exceptions::alignment_check);
    INTERRUPT_HANDLERS.lock()[18] = IrqHandler::ErrorHandler(exceptions::machine_check);
    INTERRUPT_HANDLERS.lock()[19] = IrqHandler::ErrorHandler(exceptions::simd);
    INTERRUPT_HANDLERS.lock()[20] = IrqHandler::ErrorHandler(exceptions::virtualization);

    // INTERRUPT_HANDLERS[21..29] are reserved.
    INTERRUPT_HANDLERS.lock()[30] = IrqHandler::ErrorHandler(exceptions::security);

    unsafe {
        let idt_descriptor = IdtDescriptor::new(
            ((IDT.len() * size_of::<IdtEntry>()) - 1) as u16,
            addr_of!(IDT).addr() as u64,
        );

        load_idt(&idt_descriptor);

        // Since lazy statics are initialized on the their first dereference, we have to
        // manually initialize the static as the first dereference happen in an IRQ interrupt.
        // This means that the controller will never be initialized as an IRQ interrupt requires
        // the controller to be initialized.
        lazy_static::initialize(&super::INTERRUPT_CONTROLLER);
        lazy_static::initialize(&super::PIC_CONTROLLER);
        lazy_static::initialize(&super::APIC_CONTROLLER);
    }
}

#[inline(always)]
unsafe fn load_idt(idt_descriptor: &IdtDescriptor) {
    asm!("lidt [{}]", in(reg) idt_descriptor, options(nostack));
}
