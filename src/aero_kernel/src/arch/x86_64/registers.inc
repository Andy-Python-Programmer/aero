; pops the preserved registers (the stack must have the preserved registers pushed on
; top and matches the layout of the `PreservedRegisters` structure)
%macro pop_preserved 0
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
%endmacro

; pops the scratch registers (the stack must have the scratch registers pushed on
; top and matches the layout of the `ScratchRegisters` structure)
%macro pop_scratch 0
    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rax
%endmacro

; pushes the scratch registers on the stack. The layout matches the layout of
; the `ScratchRegisters` structure though note that it does not push the RAX
; register. The caller is responsible for pushing RAX on the top of the stack.
%macro push_scratch 0
    push rcx
    push rdx
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r11
%endmacro

; pushes the preserved registers on the stack. The layout matches the layout of
; the `PreservedRegisters` structure.
%macro push_preserved 0
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
%endmacro
