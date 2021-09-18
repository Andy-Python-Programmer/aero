ENTRY(x86_64_aero_main)
OUTPUT_FORMAT(elf64-x86-64) 

/* We want to be placed in the higher half, 2MiB above 0x00 in physical memory */
KERNEL_OFFSET = 0xFFFFFFFF80200000;

SECTIONS 
{
    . = KERNEL_OFFSET;

    __kernel_start = .;

    .stivale2hdr : ALIGN(4K) {
        KEEP(*(.stivale2hdr))
    }

    /* Then place all of the other traditional executable sections afterwards... */
    . = ALIGN(4K);
    .text : {
        __text_start = .;
        *(.text .text.*)
        . = ALIGN(4096);
        __text_end = .;
    }

    . = ALIGN(4K);
    .rodata : {
        __rodata_start = .;
        *(.rodata*)
        . = ALIGN(4096);
        __rodata_end = .;
    }

    /* The kernel makes use of the tdata section for CPU-local variables... */
    . = ALIGN(4K);
    .tdata : {
        __tdata_start = .;
        KEEP(*(.tdata*))
        KEEP(*(.tbss*))
        __tdata_end = ALIGN(8);
    }

    . = ALIGN(4K);
    .data : {
        __data_start = .;
        *(.data .data.*)
        . = ALIGN(4096);
        __data_end = .;
    }

    . = ALIGN(4K);
    .kernel_modules : {
        __kernel_modules_start = .;
        KEEP(*(.kernel_modules.init))
        __kernel_modules_end = .;
    }

    . = ALIGN(4K);
    .bss : {
        __bss_start = .;
        *(.bss .bss.*)
        . = ALIGN(4096);
        __bss_end = .;
    }
    
    __kernel_end = .;

    /* 
     * Discard all of the useless sections for us. Note that Aero does do stack unwinding on panic, instead
     * of aborting, but it does not use the eh frame. See the `aero_kernel/src/unwind.rs` file for more
     * information.
     */
    /DISCARD/ : {
        *(.comment*)
        *(.eh_frame*)
        *(.gcc_except_table*)
        *(.note*)
        *(.rel.eh_frame*)
    }
}
