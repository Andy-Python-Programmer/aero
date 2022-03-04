; Copyright (C) 2021 The Aero Project Developers.
;
; This file is part of The Aero Project.
;
; Aero is free software: you can redistribute it and/or modify
; it under the terms of the GNU General Public License as published by
; the Free Software Foundation, either version 3 of the License, or
; (at your option) any later version.
;
; Aero is distributed in the hope that it will be useful,
; but WITHOUT ANY WARRANTY; without even the implied warranty of
; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
; GNU General Public License for more details.
;
; You should have received a copy of the GNU General Public License
; along with Aero. If not, see <https://www.gnu.org/licenses/>.

global jump_userland_exec
global task_spinup
global iretq_init
global fork_init

jump_userland_exec:
    push rdi ; Param: stack
    push rsi ; Param: RIP
    push rdx ; Param: RFLAGS

    cli

    pop r11
    pop rcx
    pop rsp

    swapgs
    o64 sysret

fork_init:
    cli
    swapgs
    jmp generic_iretq_init

iretq_init:
    cli
    jmp generic_iretq_init

generic_iretq_init:
    ; pop the preserved registers
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    ; pop the scratch registers
    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rax

    iretq

; extern "C" fn task_spinup(prev: &mut Context, next: &mut Context)
;
; Saves the current context into `prev` and restore the context from `next`.
task_spinup:
    ; save callee-saved registers and this must match
    ; the ordering of the fields in the `Context` struct.
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15

    mov rax, cr3    ; save CR3
    push rax

    mov [rdi], rsp	; update old context pointer with current stack pointer
    mov rsp, rsi	; switch to new stack

    pop rax         ; restore CR3
    mov cr3, rax

    ; restore callee-saved registers
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp

    ; resume the next thread
    ret
