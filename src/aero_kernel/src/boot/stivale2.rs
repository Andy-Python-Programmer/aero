/// We need to tell the stivale bootloader where we want our stack to be.
/// We are going to allocate our stack as an uninitialised array in .bss.
static STACK: [u8; 4096] = [0; 4096];

/// The stivale2 specification says we need to define a "header structure".
/// This structure needs to reside in the .stivale2hdr ELF section in order
/// for the bootloader to find it.
#[link_section = ".stivale2hdr"]
#[used]
pub static STIVALE_HEADER: StivaleHeader = StivaleHeader::new(STACK[0] as *const u8);

bitflags::bitflags! {
    /// Header flags for the stivale bootloader.
    pub struct StivaleHeaderFlags: u64 {
        /// Set if the bootloader should apply kernel address space layout randomization.
        const KASLR = 0x1;
    }
}

#[repr(C)]
union StivaleHeaderEntryPoint {
    /// The alternative entry point function.
    function: extern "C" fn(stivale_struct_addr: usize) -> !,
    padding: u64,
}

/// A stivale2 header for the bootloader.
#[repr(packed)]
#[allow(dead_code)]
pub struct StivaleHeader {
    /// The entry_point member is used to specify an alternative entry
    /// point that the bootloader should jump to instead of the executable's
    /// ELF entry point. We do not care about that so we leave it zeroed.
    entry_point: StivaleHeaderEntryPoint,
    /// [u8] pointer to the kernel stack.
    stack: *const u8,
    flags: StivaleHeaderFlags,
    /// The header structure is the root of the linked list of header tags and
    /// points to the first one in the linked list.
    tags: *const (),
}

impl StivaleHeader {
    const fn new(stack: *const u8) -> Self {
        Self {
            entry_point: StivaleHeaderEntryPoint { padding: 0 },
            stack,
            flags: StivaleHeaderFlags::empty(),
            tags: core::ptr::null(),
        }
    }
}

// SAFTEY: Send and Sync are fine because we won't be accessing the data on runtime.
unsafe impl Send for StivaleHeader {}
unsafe impl Sync for StivaleHeader {}

#[repr(C, packed)]
struct StivaleStructure {
    bootloader_brand: [u8; 64],
    bootloader_version: [u8; 64],
    tags: u64,
}

#[no_mangle]
unsafe extern "C" fn _start(boot_info_addr: u64) -> ! {
    let stivale = &*(boot_info_addr as *const StivaleStructure);

    loop {}
}
