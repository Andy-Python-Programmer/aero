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

global context_switch
global iretq_kernelinit
global jump_userland_exec

extern restore_user_tls

section .text

; This function is responsible for switching from the provided previous context to
; the new one and also save the current state in the previous context so there is a restore
; point (explains the &mut reference requirement).
context_switch:
    pushfq ; Push registers to current context.
    cli    ; Disable interrupts as we are switching stacks.

    push rbp
    push r15
    push r14
    push r13
    push r12
    push rbx

    mov rax, cr3 ; Save CR3.
    push rax

    mov [rdi], rsp ; Update old context pointer with current stack pointer.
    mov rsp,   rsi ; Switch to new stack.

    pop rax ; Restore CR3.
    mov cr3, rax

    pop rbx
    pop r12
    pop r13
    pop r14
    pop r15
    pop rbp

    popfq

    ret

; This function is responsible for switching to the kernel task stack and switching to the kernel
; task.
iretq_kernelinit:
    pop rdi
    iretq ; Leap of faith!

jump_userland_exec:
    push rdi ; Param: stack
    push rsi ; Param: RIP
    push rdx ; Param: RFLAGS

    cli
    call restore_user_tls

    pop r11
    pop rcx
    pop rsp

    o64 sysret
