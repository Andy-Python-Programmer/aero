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

mod exceptions;
mod idt;
mod irq;

pub use idt::*;

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

        $crate::prelude::intel_fn!(
            pub extern "asm" fn $name() {
                // Move rax into code's place and put code in last instead to be
                // compatible with interrupt stack.
                "xchg [rsp], rax\n",

                $crate::prelude::push_scratch!(),
                $crate::prelude::push_preserved!(),
                $crate::prelude::push_fs!(),

                // Push the error code.
                "push rax\n",

                "call map_pti\n",

                // Call the inner interrupt handler implementation.
                "mov rdi, rsp\n",
                "call __interrupt_", stringify!($name), "\n",

                "call unmap_pti\n",

                // Pop the error code.
                "add rsp, 8\n",

                $crate::prelude::pop_fs!(),
                $crate::prelude::pop_preserved!(),
                $crate::prelude::pop_scratch!(),

                "iretq\n",
            }
        );
    }
}

pub macro interrupt(pub unsafe fn $name:ident($stack:ident: &mut InterruptStack) $code:block) {
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
                "push rax\n",

                $crate::prelude::push_scratch!(),
                $crate::prelude::push_preserved!(),
                $crate::prelude::push_fs!(),

                "call map_pti\n",

                "mov rdi, rsp\n",
                "call __interrupt_", stringify!($name), "\n",

                "call unmap_pti\n",

                $crate::prelude::pop_fs!(),
                $crate::prelude::pop_preserved!(),
                $crate::prelude::pop_scratch!(),

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
