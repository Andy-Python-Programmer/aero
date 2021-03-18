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
