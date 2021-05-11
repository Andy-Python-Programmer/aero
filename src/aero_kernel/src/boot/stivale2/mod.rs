/// We need to tell the stivale bootloader where we want our stack to be.
/// We are going to allocate our stack as an uninitialised array in .bss.
static STACK: [u8; 4096] = [0; 4096];

/// The stivale2 specification says we need to define a "header structure".
/// This structure needs to reside in the .stivale2hdr ELF section in order
/// for the bootloader to find it.
#[link_section = ".stivale2hdr"]
#[used]
static STIVALE_HEADER: StivaleHeader = StivaleHeader::new(STACK[0] as *const u8)
    .tag((&FRAMEBUFFER_TAG as *const HeaderFramebufferTag).cast());

static FRAMEBUFFER_TAG: HeaderFramebufferTag = HeaderFramebufferTag::new(16);

bitflags::bitflags! {
    /// Header flags for the stivale bootloader.
    struct StivaleHeaderFlags: u64 {
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

    const fn tag(mut self, tag: *const ()) -> Self {
        self.tags = tag;

        self
    }
}

// SAFTEY: Send and Sync are fine because we won't be accessing the data on runtime.
unsafe impl Send for StivaleHeader {}
unsafe impl Sync for StivaleHeader {}

#[repr(packed)]
#[allow(dead_code)]
pub struct HeaderFramebufferTag {
    identifier: u64,
    next: *const (),
    width: u16,
    height: u16,
    bpp: u16,
}

// SAFTEY: Send and Sync are fine because we won't be accessing the data on runtime.
unsafe impl Send for HeaderFramebufferTag {}
unsafe impl Sync for HeaderFramebufferTag {}

impl HeaderFramebufferTag {
    /// Create a new header framebuffer tag that will have the bootloader determine the best
    /// resolution and bpp values.
    const fn new(bpp: u16) -> Self {
        HeaderFramebufferTag {
            identifier: 0x3ecc1bc43d0f7971,
            next: core::ptr::null(),
            width: 0,
            height: 0,
            bpp,
        }
    }
}

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
