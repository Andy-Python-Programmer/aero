;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;; BITS 64 ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
[bits 64]

jump_userland:
	mov ax, 0x1b
    mov dx, 0x23
    mov rsp, 0x00

	mov ds, ax
	mov es, ax 
	mov fs, ax 
	mov gs, ax
 
	mov rsi, rsp

    push rax
    push rsi
    push 0x200
    push rdx
    push rdi

	iretq

global jump_userland
