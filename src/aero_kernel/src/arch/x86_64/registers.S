.macro pop_preserved
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
.endm

.macro pop_scratch
    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rax
.endm

.macro push_scratch
    push rcx
    push rdx
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r11
.endm

.macro push_preserved
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
.endm
