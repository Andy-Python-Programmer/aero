section .multiboot_header

header_start:
    dd 0xe85250d6                ; Multiboot 2
    dd 0                         ; Arch 0 (protected mode i386)
    dd header_end - header_start ; Header length

    ; Checksum
    dd 0x100000000 - (0xe85250d6 + 0 + (header_end - header_start))

    dw 0 ; Type
    dw 0 ; Flags
    dd 8 ; Size
header_end: