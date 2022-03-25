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

%define USERLAND_SS         0x23
%define USERLAND_CS         0x2b

%define FMASK               0x300  ; TF | DF

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

; 64-bit SYSENTER entry point
;
; The SYSENTER mechanism performs a fast transition to the kernel.
; The new CS is loaded from the IA32_SYSENTER_CS MSR, and the new instruction
; and stack pointers are loaded from IA32_SYSENTER_EIP and IA32_SYSENTER_ESP,
; respectively. RFLAGS.IF is cleared, but other flags are unchanged.
;
; As the instruction does not save *any* state, the user is required to provide
; the return RIP and RSP in the RCX and R11 registers, respectively. These
; addresses must be canonical.
;
; The instruction expects the call number and arguments in the same registers as
; for SYSCALL.
;
section .text.x86_64_sysenter_handler
global  x86_64_sysenter_handler:function (x86_64_sysenter_handler.end - x86_64_sysenter_handler)
align   16
x86_64_sysenter_handler:
    swapgs

    ; Build the interrupt frame expected by the kernel.
    push    USERLAND_SS
    push    r11
    pushfq
    push    USERLAND_CS
    push    rcx

    ; Mask the same flags as for SYSCALL.
    ; Note that up to this pont the code can be single-stepped if the user sets TF.
    pushfq
    and     dword [rsp], 0x300
    popfq

    push    rax
    push_scratch
    push_preserved
    push    0

    ; Sore the stack pointer (interrupt frame pointer) in RBP for save keeping,
    ; and align the stack as specified by the SysV calling convention.
    mov     rbp, rsp
    and     rsp, ~0xf

    mov     rdi, rbp
    call    x86_64_do_syscall

    ; Reload the stack pointer, skipping the error code.
    lea     rsp, [rbp + 8]
    pop_preserved
    pop_scratch

    ; Restore RFLAGS
    add     rsp, 16
    popfq

    ; Move the return RIP and RSP into the registers expected by SYSEXIT.
    mov     rdx, rcx
    mov     rcx, r11

    swapgs
    o64 sysexit
.end:
