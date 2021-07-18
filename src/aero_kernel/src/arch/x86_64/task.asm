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

extern restore_user_tls

section .text

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

; This function is responsible for switching from the current context to the new one and 
; also save the current state in the previous context so there is a restore point.
task_spinup:
    ; Make sure that interrupts are disabled as we are switching 
    ; stacks.
    cli

    test rsi, rsi      ; Test if we have a new page table to load.
    jz .dont_load_cr3

    mov cr3, rsi       ; Load the new page table pointer in RSI.

.dont_load_cr3:
    mov rsp, rdi

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rbp
    pop rdx
    pop rcx
    pop rbx
    pop rax

    add rsp, 16

    iretq
