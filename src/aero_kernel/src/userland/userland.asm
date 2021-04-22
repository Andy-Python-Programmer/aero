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

; Example task that uses syscalls to print "Hello World" to
; the screen. This task is invoked after the kernel has initialized
; and will be removed after we can load ELF files from the userland/
; directory.
mission_hello_world:
    ; Exit the hello world app :D
    mov rax, 60
    mov rdi, 0
    
    syscall
    
global mission_hello_world
