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

%define TSS_TEMP_USTACK_OFF 0x1c
%define TSS_RSP0_OFF        0x04

%define USERLAND_SS         0x2b
%define USERLAND_CS         0x33

syscall_handler:
    ; swap the GS base to ensure that it points to the 
    ; kernel PCR.
    swapgs

    mov [gs:TSS_TEMP_USTACK_OFF], rsp   ; save the user stack pointer
    mov rsp, [gs:TSS_RSP0_OFF]          ; restore the kernel stack pointer
    push qword USERLAND_SS              ; push userspace SS
    push qword [gs:TSS_TEMP_USTACK_OFF] ; push userspace stack pointer
    push r11                            ; push rflags
    push qword USERLAND_CS              ; push userspace CS
    push rcx                            ; push userspace return pointer

    push rax

    ; push the scratch registers
    push rcx
    push rdx
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r11

    ; push the preserved registers
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    mov rdi, rsp

    cld
    call __inner_syscall

    cli
    call restore_user_tls

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

    ; make the sysret frame
    pop rcx
    add rsp, 8
    pop r11
    pop qword [gs:TSS_TEMP_USTACK_OFF]
    mov rsp, [gs:TSS_TEMP_USTACK_OFF]

    swapgs
    o64 sysret
