use aero_gfx::{FrameBuffer, FrameBufferInfo, PixelFormat};

use stivale::framebuffer::FramebufferTag;
use stivale::StivaleStructureInner;

extern "C" {
    fn stivale2_get_framebuffer_tag(
        stivale_struct: *mut StivaleStructureInner,
    ) -> *mut FramebufferTag;
}

#[no_mangle]
unsafe extern "C" fn __stivale_boot(stivale_struct: *mut StivaleStructureInner) {
    crate::drivers::uart_16550::init();

    let framebuffer_tag = &mut *stivale2_get_framebuffer_tag(stivale_struct);

    let framebuffer_info = FrameBufferInfo {
        byte_len: framebuffer_tag.size(),
        bytes_per_pixel: framebuffer_tag.bpp() as usize,
        horizontal_resolution: framebuffer_tag.width() as usize,
        vertical_resolution: framebuffer_tag.height() as usize,
        pixel_format: PixelFormat::BGR,
        stride: framebuffer_tag.pitch() as usize,
    };

    let mut framebuffer = FrameBuffer {
        buffer_start: framebuffer_tag.start_address() as u64,
        buffer_byte_len: framebuffer_info.byte_len,
        info: framebuffer_info,
    };

    crate::drivers::uart_16550::serial_println!("{:#x?}", framebuffer);

    crate::rendy::init(&mut framebuffer);

    crate::prelude::println!("LOL C IS AWESOME!");
    crate::drivers::uart_16550::serial_println!("LOL C IS GOOD!");
}
