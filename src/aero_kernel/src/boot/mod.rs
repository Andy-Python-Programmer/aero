use aero_gfx::FrameBuffer;
use stivale::memory::MemoryMapTag;
use x86_64::{PhysAddr, VirtAddr};

macro register_boot_protocols($($(#[$boot_meta:meta])* => $name:ident,)*) {
    $(
        $(#[$boot_meta])*
        pub mod $name;
    )*
}

#[repr(C)]
pub struct BootInfo {
    pub rsdp_address: PhysAddr,
    pub physical_memory_offset: VirtAddr,
    pub framebuffer: FrameBuffer,
    pub memory_regions: &'static MemoryMapTag,
    pub stack_top: VirtAddr,
}

register_boot_protocols!(
    #[cfg(feature = "stivale2")] => stivale2,
);
