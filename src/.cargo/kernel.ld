ENTRY(kernel_main) /* We want our entry point function to be [kernel_main] */
OUTPUT_FORMAT(elf64-x86-64) /* We want the output format to be ELF x86_64*/

/* We want to be placed in the higher half, 2MiB above 0x00 in physical memory */
KERNEL_OFFSET = 0xFFFFFFFF80200000;

SECTIONS {
    . = KERNEL_OFFSET;

    . += SIZEOF_HEADERS;
    . = ALIGN(4096);

    __kernel_start = .;


    /* Then place all of the other traditional executable sections afterwards... */
    .text : {
        __text_start = .;
        *(.text*)
        . = ALIGN(4096);
        __text_end = .;
    }

    .rodata : {
        __rodata_start = .;
        *(.rodata*)
        . = ALIGN(4096);
        __rodata_end = .;
    }

    .data : {
        __data_start = .;
        *(.data*)
        *(.got*)
        . = ALIGN(4096);
        __data_end = .;
    }

    /* The kernel makes use of the tdata section for CPU-local variables... */
    .tdata : {
        __tdata_start = .;
        *(.tdata*)
        . = ALIGN(4096);
        __tdata_end = .;
        __tbss_start = .;
        *(.tbss*)
        . += 8;
        . = ALIGN(4096);
        __tbss_end = .;
    }

    .bss : {
        __bss_start = .;
        *(.bss*)
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
