use crate::boot::*;
use aero_gfx::{FrameBuffer, FrameBufferInfo, PixelFormat};

use x86_64::{PhysAddr, VirtAddr};

use stivale::framebuffer::FramebufferTag;
use stivale::memory::MemoryMapTag;
use stivale::StivaleStructureInner;

use crate::kernel_main;

#[repr(C)]
struct StivaleBootInfo {
    framebuffer_tag: *mut FramebufferTag,
    mmap_tag: *mut MemoryMapTag,
}

#[no_mangle]
unsafe extern "C" fn __stivale_boot(stivale2_boot_info: *mut StivaleBootInfo) {
    let stivale2_boot_info = &mut *stivale2_boot_info;

    let framebuffer_tag = &mut *stivale2_boot_info.framebuffer_tag;
    let mmap_tag = &*stivale2_boot_info.mmap_tag;

    let framebuffer_info = FrameBufferInfo {
        byte_len: framebuffer_tag.size(),
        bytes_per_pixel: framebuffer_tag.bpp() as usize,
        horizontal_resolution: framebuffer_tag.width() as usize,
        vertical_resolution: framebuffer_tag.height() as usize,
        pixel_format: PixelFormat::RGB,
        stride: framebuffer_tag.pitch() as usize,
    };

    let mut framebuffer = FrameBuffer {
        buffer_start: framebuffer_tag.start_address() as u64,
        buffer_byte_len: framebuffer_info.byte_len,
        info: framebuffer_info,
    };

    let mut boot_info = BootInfo {
        rsdp_address: PhysAddr::zero(),
        physical_memory_offset: VirtAddr::new(0x00),
        framebuffer,
        memory_regions: mmap_tag,
        stack_top: VirtAddr::zero(),
    };

    kernel_main(&mut boot_info);
}
