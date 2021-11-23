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

global smp_prepare_trampoline
global smp_prepare_launch
global smp_check_ap_flag

extern kernel_ap_startup

section .data

%define smp_trampoline_size  smp_trampoline_end - smp_trampoline
smp_trampoline:              incbin "../target/smp_trampoline.real.bin"
smp_trampoline_end:

section .text

%define TRAMPOLINE_ADDR     0x1000
%define PAGE_SIZE           4096

; This function is responsible for storeing trampoline data in low
; memory and return the page index of the trampoline code.
smp_prepare_trampoline:
    ; Copy the trampoline blob to 0x1000
    mov rsi, smp_trampoline
    mov rdi, TRAMPOLINE_ADDR
    mov rcx, smp_trampoline_size
    rep movsb

    mov rax, TRAMPOLINE_ADDR / PAGE_SIZE
    ret

; This function is responsible for preparing the SMP trampoline 
; structure.

; The SMP trampoline cannot be larger then a page size so we just
; use 0x2500 + OFFSET to store the AP info.
smp_prepare_launch:
    mov byte [0x2510], 0                     ; Info: AP Ready Flag 
    mov qword [0x2520], kernel_ap_startup    ; Info: Entry point

    mov qword [0x2540], rdi                  ; Param: Page Table
    mov qword [0x2550], rsi                  ; Param: Stack Top
    mov qword [0x2560], rdx                  ; Param: AP ID
    mov dword [0x2570], ecx                  ; Param: Mode

    ret

smp_check_ap_flag:
    xor rax, rax
    mov al, byte [0x2510]
    ret
