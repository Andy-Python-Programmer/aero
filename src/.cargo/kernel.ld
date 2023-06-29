/* We want the symbol `arch_aero_main` to be our entry point */
ENTRY(arch_aero_main)

/* Define the program headers we want so the bootloader gives us the right */
/* MMU permissions */
PHDRS
{
    null    PT_NULL    FLAGS(0) ;                   /* Null segment */
    text    PT_LOAD    FLAGS((1 << 0) | (1 << 2)) ; /* Execute + Read */
    rodata  PT_LOAD    FLAGS((1 << 2)) ;            /* Read only */
    data    PT_LOAD    FLAGS((1 << 1) | (1 << 2)) ; /* Write + Read */
}

SECTIONS
{
    /* We wanna be placed in the topmost 2GiB of the address space, for optimisations */
    /* and because that is what the Limine spec mandates. */
    /* Any address in this region will do, but often 0xffffffff80000000 is chosen as */
    /* that is the beginning of the region. */
    . = 0xffffffff80000000;

    .text : {
        *(.text .text.*)
    } :text

    /* Move to the next memory page for .rodata */
    . += CONSTANT(MAXPAGESIZE);

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    .cpu_local : {
        __cpu_local_start = .;
        KEEP(*(.cpu_local_self_ptr));
        KEEP(*(.cpu_local_tss));
        KEEP(*(.cpu_local));
        __cpu_local_end = .;
    }

    /* Move to the next memory page for .data */
    . += CONSTANT(MAXPAGESIZE);

    .data : {
        *(.data .data.*)
    } :data

    .kernel_modules : {
        __kernel_modules_start = .;
        KEEP(*(.kernel_modules.init))
        __kernel_modules_end = .;
    }

    .bss : {
        *(COMMON)
        *(.bss .bss.*)
    } :data
}