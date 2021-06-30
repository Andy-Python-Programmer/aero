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

section .data

%define smp_trampoline_size  smp_trampoline_end - smp_trampoline
smp_trampoline:              incbin "../target/smp_trampoline.bin"
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
smp_prepare_launch:
    mov byte [0x510], 0    
    mov qword [0x520], rdi ; Param: Entry Point
    mov qword [0x540], rsi ; Param: Page Table
    mov qword [0x550], rdx ; Param: Stack Top
    mov qword [0x560], rcx ; Param: AP ID

    ret

smp_check_ap_flag:
    xor rax, rax
    mov al, byte [0x510]
    ret
