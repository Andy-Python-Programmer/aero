use core::fmt::Display;

use x86_64::{
    registers::{control::Cr2, rflags::RFlags},
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

struct InterruptStack(&'static mut InterruptStackFrame);

impl From<&'static mut InterruptStackFrame> for InterruptStack {
    fn from(stack: &'static mut InterruptStackFrame) -> Self {
        Self(stack)
    }
}

impl Display for InterruptStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Instruction Pointer -> {:?}", self.0.instruction_pointer)?;
        writeln!(f, "Code Segment -> {:#X}", self.0.code_segment)?;
        writeln!(
            f,
            "CPU Flags -> {:?}",
            RFlags::from_bits_truncate(self.0.cpu_flags)
        )?;
        writeln!(f, "Stack Pointer -> {:?}", self.0.stack_pointer)?;
        writeln!(f, "Stack Segment -> {:#X}", self.0.stack_segment)?;

        Ok(())
    }
}

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
    stack_frame: &'static mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let stack_frame = InterruptStack::from(stack_frame);
    let accessed_address = Cr2::read();

    panic!(
        "EXCEPTION: Page Fault\n\nAccessed Address: {:?}\nError Code: {:?}\n\n{}",
        accessed_address, error_code, stack_frame
    )
}
