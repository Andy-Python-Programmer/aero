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
global sysretq_userinit
global iretq_kernelinit

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

; This function is responsible for stashing the kernel stack and switching to the task stack,
; and then jumping to userland.
sysretq_userinit:
    ; After pushing all of the required registers on the stack
    ; disable interrupts as we are swaping stacks. Interrupts are
    ; automatically enabled after `sysretq`.
    cli
    call restore_user_tls

    pop r11 ; Restore RFLAGS.
    pop rcx ; Restore RIP.

    push rdx

    swapgs

    mov rdx, rsp
    add rdx, 16        ; Skip RDX and user RSP currently on the stack.
    mov [gs:0x04], rdx ; Stash kernel stack.

    swapgs
    pop rdx
    pop rsp ; Restore user stack.

    sysretq

; This function is responsible for switching to the kernel task stack and switching to the kernel
; task.
iretq_kernelinit:
    pop rdi
    iretq ; Leap of faith!
