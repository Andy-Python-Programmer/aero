//! ## Notes
//!
//! External C file is required as rust does not like us using static's in
//! const functions.

#include <stivale2.h>
#include <stdint.h>
#include <stddef.h>

/// We need to tell the stivale bootloader where we want our stack to be.
/// We are going to allocate our stack as an uninitialised array in .bss.
static uint8_t stack[4096];

static struct stivale2_header_tag_framebuffer framebuffer_hdr_tag = {
    .tag = {
        .identifier = STIVALE2_HEADER_TAG_FRAMEBUFFER_ID,
        .next = 0,
    },
    .framebuffer_width = 0,
    .framebuffer_height = 0,
    .framebuffer_bpp = 0,
};

/// The stivale2 specification expects us to define a "header structure".
/// This structure needs to reside in the .stivale2hdr ELF section in order
/// for the bootloader to find it.
__attribute__((section(".stivale2hdr"), used)) static struct stivale2_header stivale_hdr = {
    // The entry_point member is used to specify an alternative entry
    // point that the bootloader should jump to instead of the executable's
    // ELF entry point. We do not care about that so we leave it zeroed.
    .entry_point = 0,
    // Let's tell the bootloader where our stack is.
    // We need to add the sizeof(stack) since in x86(_64) the stack grows
    // downwards.
    .stack = (uintptr_t)stack + sizeof(stack),
    // No flags are currently defined as per spec and should be left to 0.
    .flags = 0,
    // This header structure is the root of the linked list of header tags and
    // points to the first one in the linked list.
    .tags = (uintptr_t)&framebuffer_hdr_tag,
};

// We will now write a helper function which will allow us to scan for tags
// that we want FROM the bootloader (structure tags).
void *stivale2_get_tag(struct stivale2_struct *stivale2_struct, uint64_t id)
{
    struct stivale2_tag *current_tag = (void *)stivale2_struct->tags;

    for (;;)
    {
        // If the tag pointer is NULL (end of linked list), we did not find
        // the tag. Return NULL to signal this.
        if (current_tag == NULL)
        {
            return NULL;
        }

        // Check whether the identifier matches. If it does, return a pointer
        // to the matching tag.
        if (current_tag->identifier == id)
        {
            return current_tag;
        }

        // Get a pointer to the next tag in the linked list and repeat.
        current_tag = (void *)current_tag->next;
    }
}

void loop()
{
    __asm__ volatile("cli");

    for (;;)
    {
        __asm__ volatile("hlt");
    }
}

struct stivale2_struct_tag_framebuffer *stivale2_get_framebuffer_tag(struct stivale2_struct *stivale2_struct)
{
    struct stivale2_struct_tag_framebuffer *framebuffer_tag;

    framebuffer_tag = stivale2_get_tag(stivale2_struct, STIVALE2_STRUCT_TAG_FRAMEBUFFER_ID);

    if (framebuffer_tag == NULL)
        return NULL;

    return framebuffer_tag;
}

extern void __stivale_boot();

/// Entry point function for our kernel.
///
/// ## Notes
/// As we are in C the `#[no_mangle]` attribute is not required.
void _start(struct stivale2_struct *stivale2_struct)
{
    __stivale_boot(stivale2_struct);

    /*
     * There is nothing that we can really do in this situation. So
     * we loop for ever!
     */
    loop();
}
