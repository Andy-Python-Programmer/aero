.intel_syntax noprefix

# **Notes**: 0x00: The Kernel Null Segment.
#            0x10: The Kernel Data Segment.
#            0x08: The Kernel Code Segment.

# Load the GDT and set all of the segments.
LoadGDT:
    # Use the `lgdt` Load GDT instruction to set the new GDT.
    # The rdi register contains the first argument of the function.
    lgdt [rdi]

    mov ax, 0x10
    
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    pop rdi
    
    mov rax, 0x08
    
    push rax
    push rdi
    
    retfq

.global LoadGDT