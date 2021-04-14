use aero_boot::FrameBufferInfo;

use x86_64::PhysAddr;

/// Required system information that should be queried from the BIOS or UEFI firmware.
#[derive(Debug, Copy, Clone)]
pub struct SystemInfo {
    /// Start address of the pixel-based framebuffer.
    pub framebuffer_address: PhysAddr,
    /// Information about the framebuffer, including layout and pixel format.
    pub framebuffer_info: FrameBufferInfo,
    /// Address of the _Root System Description Pointer_ structure of the ACPI standard.
    pub rsdp_address: Option<PhysAddr>,
}

pub fn load_and_switch_to_kernel(system_info: SystemInfo) {}
