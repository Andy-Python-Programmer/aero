global _start

section .text
_start:
    mov rdx, len
    mov rcx, msg
    mov rbx, 1
    mov rax, 1 ; SYS_WRITE

    syscall

    mov rbx, 0
    mov rax, 5 ; SYS_EXIT
    syscall

    ud2 ; unreacheable

section .data
msg db      "Hello, world!", 0xa
len equ     $ - msg