use x86_64::{registers::control::Cr2, structures::idt::PageFaultErrorCode};

use super::InterruptStackFrame;
use crate::interrupt;

macro_rules! interrupt_exception {
    ($name:ident, $error:expr) => {
        interrupt!($name, { panic!($error) });
    };
}

interrupt_exception!(divide_by_zero, "Division by zero");
interrupt_exception!(debug, "Debug");
interrupt_exception!(non_maskable, "Non Maskable");
interrupt_exception!(breakpoint, "Breakpoint");
interrupt_exception!(overflow, "Stack Overflow");
interrupt_exception!(bound_range, "Out of Bounds");
interrupt_exception!(invalid_opcode, "Invalid Opcode");
interrupt_exception!(device_not_available, "Device not Avaliable");
interrupt_exception!(double_fault, "Double Fault");
interrupt_exception!(invalid_tss, "Invalid TSS");
interrupt_exception!(segment_not_present, "Segment not Present");
interrupt_exception!(stack_segment, "Stack Segment Fault");
interrupt_exception!(protection, "Protection Fault");
interrupt_exception!(fpu_fault, "FPU floating point fault");
interrupt_exception!(alignment_check, "Alignment check fault");
interrupt_exception!(machine_check, "Machine check fault");
interrupt_exception!(simd, "SIMD floating point fault");
interrupt_exception!(virtualization, "Virtualization fault");
interrupt_exception!(security, "Security exception");

pub extern "x86-interrupt" fn page_fault(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read();

    panic!(
        "EXCEPTION: Page Fault\n\nAccessed Address: {:?}\nError Code: {:?}\n\n{:#?}",
        accessed_address, error_code, stack_frame
    )
}
