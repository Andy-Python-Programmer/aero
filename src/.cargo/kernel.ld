ENTRY(kernel_main)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS {
    /* 
     * We want the kernel be placed in the higher half, 2MiB above 0 
     * in physical memory. 
     */
    KERNEL_VMA = 0xFFFFFFFF80000000;
    . = 0x100000;

    __kernel_start = .;

    .text : AT(ADDR(.text) - KERNEL_VMA) {
        __text_start = .;
        *(.text*)
        . = ALIGN(4096);
        __text_end = .;
    }

    .rodata : AT(ADDR(.rodata) - KERNEL_VMA) {
        __rodata_start = .;
        *(.rodata*)
        . = ALIGN(4096);
        __rodata_end = .;
    }

    .data : AT(ADDR(.data) - KERNEL_VMA) {
        __data_start = .;
        *(.data*)
        *(.got*)
        . = ALIGN(4096);
        __data_end = .;
    }

    .tdata : AT(ADDR(.tdata) - KERNEL_VMA) {
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

    .bss : AT(ADDR(.bss) - KERNEL_VMA) {
        __bss_start = .;
        *(.bss*)
        . = ALIGN(4096);
        __bss_end = .;
    }
    
    . = ALIGN(4096);
    
    __kernel_end = .;

    /DISCARD/ : {
        *(.comment*)
        *(.eh_frame*)
        *(.gcc_except_table*)
        *(.note*)
        *(.rel.eh_frame*)
    }
}
