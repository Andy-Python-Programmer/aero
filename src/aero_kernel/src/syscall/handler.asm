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

extern restore_kernel_fs_base_locked
extern restore_user_tls
extern __inner_syscall

global syscall_handler

syscall_handler:
    swapgs
    mov [gs:0x1C], rsp  ; Temporarily save user stack
    mov rsp, [gs:0x04]   ; Set kernel stack

    sub rsp, 0x08

    push rax
    mov rax, qword [gs:0x1C]
    mov qword [gs:0x1C], 0
    mov [rsp + 0x08], rax
    pop rax

    push rcx
    push r11

    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax
    mov rax, cr2
    push rax

    mov rdi, rsp          ; Param: pointer to the syscall frame
    add rdi, 128
    mov rsi, rsp          ; Param: pointer to the registers frame

    cld
    call __inner_syscall  ; Invoke the inner syscall handler implementation

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

    pop r11           ; Restore RFLAGS
    pop rcx           ; Restore RIP

    push rdx
    mov rdx, rsp
    add rdx, 16       ; Skip RDX and user RSP currently on the stack
    mov [gs:4], rdx   ; Stash kernel stack

    pop rdx
    pop rsp           ; Restore user stack

    swapgs
    o64 sysret
