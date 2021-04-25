;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;; BITS 64 ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
[bits 64]

; Defined in syscall.rs
extern SYSCALL_HANDLER_TABLE
extern log_debug

; Calls the appropriate syscall function indexed by the syscall id in [SYSCALL_HANDLER_TABLE].
syscall_handler:
    ; Save the context
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11

    ; Save ds/es and set them to the kernel data descriptior.
    mov rcx, ds
    
    push rcx
    mov rcx, es
    push rcx

    mov rcx, 0x10
    mov ds, rcx
    mov es, rcx

    ; Switch to the kernel stack.
    swapgs

    mov rcx, rsp
    rdgsbase rsp

    push rcx

    ; Copy the 4th argument to rcx.
    mov rcx, r10

    sti
    call [SYSCALL_HANDLER_TABLE + 8 * rax]
    cli

    ; Switch to the user stack.
    pop rcx
    mov rsp, rcx
    swapgs

    pop rcx
    mov es, rcx
    pop rcx
    
    mov ds, rcx
    
    ; Restore the context.
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx

    sysretq

sys_unimplemented:
    ret

global syscall_handler
global sys_unimplemented
