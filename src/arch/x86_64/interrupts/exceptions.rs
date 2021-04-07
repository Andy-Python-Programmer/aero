use x86_64::{
    registers::control::Cr2,
    structures::idt::{InterruptStackFrame, PageFaultErrorCode},
};

use crate::interrupt;

interrupt!(divide_by_zero, { panic!("Division by zero") });
interrupt!(debug, { panic!("Debug") });
interrupt!(non_maskable, { panic!("Non Maskable") });
interrupt!(breakpoint, { panic!("Breakpoint") });
interrupt!(overflow, { panic!("Stack Overflow") });
interrupt!(bound_range, { panic!("Out of Bounds") });
interrupt!(invalid_opcode, { panic!("Invalid Opcode") });
interrupt!(device_not_available, { panic!("Device not Avaliable") });
interrupt!(double_fault, { panic!("Double Fault") });
interrupt!(invalid_tss, { panic!("Invalid TSS") });
interrupt!(segment_not_present, { panic!("Segment not Present") });
interrupt!(stack_segment, { panic!("Stack Segment Fault") });
interrupt!(protection, { panic!("Protection Fault") });
interrupt!(fpu_fault, { panic!("FPU floating point fault") });
interrupt!(alignment_check, { panic!("Alignment check fault") });
interrupt!(machine_check, { panic!("Machine check fault") });
interrupt!(simd, { panic!("SIMD floating point fault") });
interrupt!(virtualization, { panic!("Virtualization fault") });
interrupt!(security, { panic!("Security exception") });

pub extern "x86-interrupt" fn page_fault(
    stack_frame: &'static mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed_address = Cr2::read();

    panic!(
        "EXCEPTION: Page Fault\n\nAccessed Address: {:?}\nError Code: {:?}\n\n{:?}",
        accessed_address, error_code, stack_frame
    )
}
