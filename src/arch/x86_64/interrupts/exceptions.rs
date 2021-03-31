use x86_64::structures::idt::InterruptStackFrame;

use super::PageFaultErrorCode;
use crate::arch::cpu::CPUState;

pub(crate) extern "x86-interrupt" fn divide_by_zero() {
    panic!("Division by zero")
}

pub(crate) extern "x86-interrupt" fn debug() {
    panic!("Debug")
}

pub(crate) extern "x86-interrupt" fn non_maskable() {
    panic!("Non Maskable")
}

pub(crate) extern "x86-interrupt" fn breakpoint() {
    panic!("Breakpoint")
}

pub(crate) extern "x86-interrupt" fn overflow() {
    panic!("Stack Overflow")
}

pub(crate) extern "x86-interrupt" fn bound_range() {
    panic!("Out of Bounds")
}

pub(crate) extern "x86-interrupt" fn invalid_opcode() {
    panic!("Invalid Opcode")
}

pub(crate) extern "x86-interrupt" fn device_not_available() {
    panic!("Device not Avaliable")
}

pub(crate) extern "x86-interrupt" fn double_fault() {
    panic!("Double Fault")
}

pub(crate) extern "x86-interrupt" fn invalid_tss() {
    panic!("Invalid TSS")
}

pub(crate) extern "x86-interrupt" fn page_fault(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let cpu = CPUState::new();

    panic!(
        "Page Fault {:?}\n\n{:#x?}{:#x?}",
        error_code, cpu, stack_frame
    )
}
