;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;; BITS 64 ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
[bits 64]

jump_userland:
    mov ax, (4 * 8) | 3

    mov ds, ax
    mov es, ax 
    mov fs, ax 
    mov gs, ax
 
    mov rax, rsp

    ; Set up the stack frame `iret` expects.
    push (4 * 8) | 3 ; Data selector
    push rax
    
    pushf ; eflags

    push (3 * 8) | 3 ; Code selector (ring 3 code with bottom 2 bits set for ring 3)
    push rdi

    iret

global jump_userland
