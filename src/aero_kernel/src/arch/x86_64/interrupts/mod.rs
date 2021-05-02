mod exceptions;
mod idt;
mod irq;

pub use idt::*;

pub macro interrupt_error_stack(fn $name:ident($stack:ident: InterruptErrorStack) $code:block) {
    paste::item! {
        #[no_mangle]
        unsafe extern "C" fn [<__interrupt_ $name>](stack: *mut $crate::arch::interrupts::InterruptErrorStack) {
            #[inline(always)]
            #[allow(unused)] // Unused variable ($stack).
            fn inner($stack: &mut $crate::arch::interrupts::InterruptErrorStack) {
                $code
            }

            inner(&mut *stack);
        }

        $crate::utils::intel_fn!(pub fn $name() {
            // Move rax into code's place and put code in last instead to be
            // compatible with interrupt stack.
            "xchg [rsp], rax\n",

            $crate::utils::push_scratch!(),
            $crate::utils::push_preserved!(),

            // Push the error code.
            "push rax\n",

            // Call the inner interrupt handler implementation.
            "mov rdi, rsp\n",
            "call __interrupt_", stringify!($name), "\n",

            // Pop the error code.
            "add rsp, 8\n",

            $crate::utils::pop_preserved!(),
            $crate::utils::pop_scratch!(),

            "iretq\n",
        });
    }
}

pub macro interrupt {
    (pub fn $name:ident() $code:block) => {
        paste::item! {
            $crate::utils::intel_fn!(pub fn $name() {
                $crate::utils::push_scratch!(),
                $crate::utils::push_preserved!(),

                "mov rdi, rsp\n",
                "call __interrupt_", stringify!($name), "\n",

                $crate::utils::pop_preserved!(),
                $crate::utils::pop_scratch!(),
            });
        }
    },

    (pub unsafe fn $name:ident() $code:block) => {

    },
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
