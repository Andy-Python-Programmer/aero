use super::interrupt_error_stack;
use crate::unwind;
use x86_64::registers::control::Cr2;

macro interrupt_exception(fn $name:ident() => $message:expr) {
    super::interrupt_error_stack!(
        fn $name(stack: &mut InterruptErrorStack) {
            log::error!($message);

            $crate::unwind::exception_begin_unwind();
        }
    );
}

interrupt_exception!(fn divide_by_zero() => "Division by zero");
interrupt_exception!(fn debug() => "Debug");
interrupt_exception!(fn non_maskable() => "Non Maskable");
interrupt_exception!(fn overflow() => "Stack Overflow");
interrupt_exception!(fn bound_range() => "Out of Bounds");
interrupt_exception!(fn invalid_opcode() => "Invalid Opcode");
interrupt_exception!(fn device_not_available() => "Device not Avaliable");
interrupt_exception!(fn double_fault() => "Double Fault");
interrupt_exception!(fn invalid_tss() => "Invalid TSS");
interrupt_exception!(fn segment_not_present() => "Segment not Present");
interrupt_exception!(fn stack_segment() => "Stack Segment Fault");
interrupt_exception!(fn protection() => "Protection Fault");
interrupt_exception!(fn fpu_fault() => "FPU floating point fault");
interrupt_exception!(fn alignment_check() => "Alignment check fault");
interrupt_exception!(fn machine_check() => "Machine check fault");
interrupt_exception!(fn simd() => "SIMD floating point fault");
interrupt_exception!(fn virtualization() => "Virtualization fault");
interrupt_exception!(fn security() => "Security exception");

interrupt_error_stack!(
    fn breakpoint(stack: &mut InterruptErrorStack) {
        /*
         * We will need to prevent RIP from going out of sync with
         * instructions.
         *
         * So we will set the RIP to RIP - 1, pointing to the int3
         * instruction.
         */
        (*stack).stack.iret.rip -= 1;
    }
);

interrupt_error_stack!(
    fn page_fault(stack: &mut InterruptErrorStack) {
        let accessed_address = Cr2::read();

        log::error!(
            "EXCEPTION: Page Fault\n\nAccessed Address: {:?}\nStack: {:#x?}",
            accessed_address,
            stack
        );

        unwind::exception_begin_unwind();
    }
);
