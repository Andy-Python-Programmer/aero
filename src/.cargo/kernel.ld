ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS {
    . = 1M;

    __kernel_start = .;

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

    .tdata : {
       __tdata_start = .;
        KEEP(*(.tdata*))
        KEEP(*(.tbss*))
        __tdata_end = ALIGN(8);
    }

    .bss : {
        __bss_start = .;
        *(.bss*)
        . = ALIGN(4096);
        __bss_end = .;
    }
    
    . = ALIGN(0x1000);
    __kernel_end = .;

    /DISCARD/ : {
        *(.comment*)
        *(.eh_frame*)
        *(.gcc_except_table*)
        *(.note*)
        *(.rel.eh_frame*)
    }
}
