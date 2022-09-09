#[repr(C)]
pub struct InterruptStack {
    spsr: u64,
    pc: u64,
    x31: u64,
    x30: u64,
    x29: u64,
    x28: u64,
    x27: u64,
    x26: u64,
    x25: u64,
    x24: u64,
    x23: u64,
    x22: u64,
    x21: u64,
    x20: u64,
    x19: u64,
    x18: u64,
    x17: u64,
    x16: u64,
    x15: u64,
    x14: u64,
    x13: u64,
    x12: u64,
    x11: u64,
    x10: u64,
    x9: u64,
    x8: u64,
    x7: u64,
    x6: u64,
    x5: u64,
    x4: u64,
    x3: u64,
    x2: u64,
    sp: u64,
    x1: u64,
    x0: u64,
}

pub fn allocate_vector() -> u8 {
    unimplemented!()
}

pub fn register_handler(vector: u8, handler: fn(&mut InterruptStack)) {
    unimplemented!()
}

pub unsafe fn disable_interrupts() {
    unimplemented!()
}

pub fn is_enabled() -> bool {
    unimplemented!()
}

pub unsafe fn enable_interrupts() {
    unimplemented!()
}

pub unsafe fn halt() {
    asm!("wfi");
}
