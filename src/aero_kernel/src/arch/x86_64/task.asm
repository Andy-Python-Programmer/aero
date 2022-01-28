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
global sysret_fork_init

extern restore_user_tls

jump_userland_exec:
    push rdi ; Param: stack
    push rsi ; Param: RIP
    push rdx ; Param: RFLAGS

    cli
    call restore_user_tls

    pop r11
    pop rcx
    pop rsp

    swapgs

    o64 sysret

iretq_init:
    pop rdi
    iretq

sysret_fork_init:
    cli
    call restore_user_tls

    pop rax
    mov cr2, rax
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop rbp
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15

    pop r11     ; Restore rflags
    pop rcx     ; Restore rip

    push rdx

    mov rdx, rsp
    add rdx, 16            ; Skip RDX and user RSP currently on the stack
    mov [gs:0x04], rdx     ; Stash kernel stack

    pop rdx
    pop rsp                ; Restore user stack

    swapgs

    o64 sysret

; extern "C" fn task_spinup(prev: &mut Context, next: &mut Context)
;
; Saves the current context into `prev` and restore the context from `next`.
task_spinup:
    pushfq

    cli

    push rbp
    push r15
    push r14
    push r13
    push r12
    push rbx

    mov rax, cr3    ; Save CR3
    push rax

    mov [rdi], rsp	; Update old context pointer with current stack pointer
    mov rsp, rsi	; Switch to new stack

    pop rax         ; Restore CR3
    mov cr3, rax

    pop rbx
    pop r12
    pop r13
    pop r14
    pop r15
    pop rbp

    popfq

    ; Resume the next thread.
    ret
