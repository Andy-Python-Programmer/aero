use stivale::StivaleStructureInner;

#[no_mangle]
extern "C" fn __stivale_boot(stivale_struct: *mut StivaleStructureInner) {
    crate::drivers::uart_16550::init();
    crate::drivers::uart_16550::serial_println!("LOL C IS GOOD!");
}
