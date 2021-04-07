//! The IDT is similar to the Global Descriptor Table in structure.
//!
//! **Notes**: <https://wiki.osdev.org/Interrupt_Descriptor_Table>

/// Declare an IDT of 256 entries.
/// Although not all entries are used, the rest exists as if any undefined IDT entry is hit,
/// it will cause an "Unhandled Interrupt" exception.
pub(crate) const IDT_ENTRIES: usize = 256;

pub(crate) const PIC1_COMMAND: u16 = 0x20;
pub(crate) const PIC1_DATA: u16 = 0x21;

pub(crate) const PIC2_COMMAND: u16 = 0xA0;
pub(crate) const PIC2_DATA: u16 = 0xA1;

pub(crate) const PIC_EOI: u8 = 0x20;

pub(crate) const ICW1_INIT: u8 = 0x10;
pub(crate) const ICW1_ICW4: u8 = 0x01;
pub(crate) const ICW4_8086: u8 = 0x01;

pub(crate) type IDTInterruptHandlerFn =
    unsafe extern "x86-interrupt" fn(&'static mut InterruptStackFrame);

static mut IDT: [IDTEntry; IDT_ENTRIES] = [IDTEntry::null(); IDT_ENTRIES];

use bitflags::bitflags;
use core::mem::size_of;
use x86_64::structures::idt::InterruptStackFrame;

use crate::utils::io;

bitflags! {
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
struct IDTDescriptor {
    size: u16,
    offset: u64,
}

impl IDTDescriptor {
    /// Create a new IDT descriptor.
    #[inline]
    const fn new(size: u16, offset: u64) -> Self {
        Self { size, offset }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct IDTEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_middle: u16,
    offset_hi: u32,
    ignore: u32,
}

impl IDTEntry {
    /// Create a new IDT entry with all values defaulted to 0, ie `null`.
    const fn null() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_middle: 0,
            offset_hi: 0,
            ignore: 0,
        }
    }

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
    pub(crate) fn set_function(&mut self, handler: IDTInterruptHandlerFn) {
        self.set_flags(IDTFlags::PRESENT | IDTFlags::RING_0 | IDTFlags::INTERRUPT);
        self.set_offset(8, handler as usize);
    }
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
        IDT[32].set_function(super::irq::pit);
        IDT[33].set_function(super::irq::keyboard);
        IDT[44].set_function(super::irq::mouse);

        IDT[0x80].set_function(crate::syscall::syscall);
        IDT[0x80].set_flags(IDTFlags::PRESENT | IDTFlags::RING_3 | IDTFlags::INTERRUPT);

        let idt_descriptor = IDTDescriptor::new(
            ((IDT.len() * size_of::<IDTEntry>()) - 1) as u16,
            (&IDT as *const _) as u64,
        );

        load_idt(&idt_descriptor as *const _);
        load_pic();
    }
}

#[inline]
unsafe fn load_idt(idt_descriptor: *const IDTDescriptor) {
    asm!("lidt [{}]", in(reg) idt_descriptor, options(nostack));
}

#[inline(always)]
pub unsafe fn disable_interrupts() {
    asm!("cli");
}

#[inline(always)]
pub unsafe fn enable_interrupts() {
    asm!("sti");
}

/// Halt the CPU. Will resume when interrupt is recieved if interrupts are
/// enabled.
#[inline(always)]
pub unsafe fn halt() {
    asm!("hlt", options(nomem, nostack));
}

#[inline]
pub unsafe fn end_pic1() {
    io::outb(PIC1_COMMAND, PIC_EOI);
}

#[inline]
pub unsafe fn end_pic2() {
    io::outb(PIC2_COMMAND, PIC_EOI);
    io::outb(PIC1_COMMAND, PIC_EOI);
}

unsafe fn load_pic() {
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
}

#[macro_export]
macro_rules! interrupt {
    ($name:ident, $code:block) => {
        #[allow(unused)]
        pub extern "x86-interrupt" fn $name(stack_frame: &'static mut InterruptStackFrame) {
            $code
        }
    };

    ($name:ident, unsafe $code:block) => {
        #[allow(unused)]
        pub unsafe extern "x86-interrupt" fn $name(stack_frame: &'static mut InterruptStackFrame) {
            $code
        }
    };
}
