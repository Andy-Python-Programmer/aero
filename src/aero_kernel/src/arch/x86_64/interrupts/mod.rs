mod exceptions;
mod idt;
mod irq;

pub use idt::*;

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

/// Enables interrupts and then halts the CPU.
#[inline(always)]
pub unsafe fn enable_interrupts_and_halt() {
    enable_interrupts();
    halt();
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
