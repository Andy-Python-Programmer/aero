use stivale::framebuffer::FramebufferTag;
use stivale::StivaleStructureInner;

extern "C" {
    fn stivale2_get_framebuffer_tag(
        stivale_struct: *mut StivaleStructureInner,
    ) -> *mut FramebufferTag;
}

#[no_mangle]
unsafe extern "C" fn __stivale_boot(stivale_struct: *mut StivaleStructureInner) {
    let framebuffer_tag = &mut *stivale2_get_framebuffer_tag(stivale_struct);

    crate::drivers::uart_16550::init();
    crate::drivers::uart_16550::serial_println!("LOL C IS GOOD!");
}
