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

bits 64

%include "registers.inc"

extern x86_64_do_syscall
global x86_64_syscall_handler

%define TSS_TEMP_USTACK_OFF 0x1c
%define TSS_RSP0_OFF        0x04

%define USERLAND_SS         0x2b
%define USERLAND_CS         0x33

; 64-bit SYSCALL instruction entry point. The instruction supports
; to to 6 arguments in registers.
;
; Registers state on entry:
; RAX - system call number
; RCX - return address
; R11 - saved flags (note: R11 is callee-clobbered register in C ABI)
; RDI - argument 1
; RSI - argument 2
; RDX - argument 3
; R10 - argument 4 (needs to be moved to RCX to conform to C ABI)
; R8  - argument 5
; R9  - argument 6
;
; (note: R12..R15, RBP, RBX are callee-preserved in C ABI)
;
; The instruction saves the RIP to RCX, cleares RFLAGS.RF then saves
; RFLAGS to R11. Followed by, it loads the new SS, CS, and RIP from
; previously programmed MSRs.
;
; The instruction also does not save anything on the stack and does
; *not* change the RSP.
x86_64_syscall_handler:
    ; swap the GS base to ensure that it points to the 
    ; kernel PCR.
    swapgs

    mov [gs:TSS_TEMP_USTACK_OFF], rsp   ; save the user stack pointer
    mov rsp, [gs:TSS_RSP0_OFF]          ; restore the kernel stack pointer
    push qword USERLAND_SS              ; push userspace SS
    push qword [gs:TSS_TEMP_USTACK_OFF] ; push userspace stack pointer
    push r11                            ; push RFLAGS
    push qword USERLAND_CS              ; push userspace CS
    push rcx                            ; push userspace return pointer

    push rax
    push_scratch
    push_preserved

    ; push a "fake" error code to match with the layout of the
    ; `InterruptErrorStack` structure.
    push 0

    mov rdi, rsp

    cld
    call x86_64_do_syscall
    cli

    ; pop the "fake" error code
    add rsp, 8

    pop_preserved
    pop_scratch

    ; make the sysret frame
    pop rcx
    add rsp, 8
    pop r11

    pop rsp

    swapgs
    o64 sysret
