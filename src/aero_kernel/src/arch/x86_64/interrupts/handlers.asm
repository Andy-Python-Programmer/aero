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

global interrupt_handlers

extern generic_interrupt_handler
extern interrupt_check_signals_error_stack

%macro make_interrupt_handler 2
[global interrupt_handler_%1]
interrupt_handler_%1:
%if %2 == 0
    push 0
%endif

    ; `swapgs` if the interrupt has originated from ring 3 since
    ; we want the GS base to point to the kernel per-cpu data.
    ;
    ; Check whether the last two bits RSP+24 (code segment) are 
    ; equal to zero.
    test qword [rsp + 16], 0x3
    ; skip the SWAPGS instruction if CS & 0b11 == 0b00.
    jz .dont_swapgs
    lfence
    swapgs
    .dont_swapgs:

    xchg [rsp], rax

    ; note: RAX is now on the top of the stack.
    push_scratch
    push_preserved

    push rax

    mov rdi, %1
    mov rsi, rsp

    ; call the generic interrupt handler
    call generic_interrupt_handler

    ; pop the error code
    add rsp, 8

    pop_preserved
    pop_scratch

    ; `swapgs` if the interrupt has originated from ring 3 since currently
    ; the GS base points to the kernel per-cpu data.
    ;
    ; Check whether the last two bits RSP+8 (code segment) are 
    ; equal to zero.
    test qword [rsp + 8], 0x3
    ; skip the SWAPGS instruction if CS & 0b11 == 0b00.
    jz .dont_swapgs_again
    lfence
    swapgs
    .dont_swapgs_again:
    
    ; voila! we're done!
    iretq
%endmacro

%macro interrupt_handler_no_error_code 1
    make_interrupt_handler %1, 0
%endmacro

%macro interrupt_handler_error_code 1
    make_interrupt_handler %1, 1
%endmacro

interrupt_handler_no_error_code 0
interrupt_handler_no_error_code 1
interrupt_handler_no_error_code 2
interrupt_handler_no_error_code 3
interrupt_handler_no_error_code 4
interrupt_handler_no_error_code 5
interrupt_handler_no_error_code 6
interrupt_handler_no_error_code 7

interrupt_handler_error_code 8
interrupt_handler_error_code 10
interrupt_handler_error_code 11
interrupt_handler_error_code 12
interrupt_handler_error_code 13
interrupt_handler_error_code 14

interrupt_handler_no_error_code 16

interrupt_handler_error_code 17

interrupt_handler_no_error_code 18
interrupt_handler_no_error_code 19
interrupt_handler_no_error_code 20

interrupt_handler_error_code 30

%assign i 32
%rep 224
    interrupt_handler_no_error_code i
%assign i i + 1
%endrep

section .rodata

interrupt_handlers:
    dq interrupt_handler_0
    dq interrupt_handler_1
    dq interrupt_handler_2
    dq interrupt_handler_3
    dq interrupt_handler_4
    dq interrupt_handler_5
    dq interrupt_handler_6
    dq interrupt_handler_7
    dq interrupt_handler_8
    dq 0
    dq interrupt_handler_10
    dq interrupt_handler_11
    dq interrupt_handler_12
    dq interrupt_handler_13
    dq interrupt_handler_14
    dq 0
    dq interrupt_handler_16
    dq interrupt_handler_17
    dq interrupt_handler_18
    dq interrupt_handler_19
    dq interrupt_handler_20
    dq 0
    dq 0
    dq 0
    dq 0
    dq 0
    dq 0
    dq 0
    dq 0
    dq 0
    dq interrupt_handler_30
    dq 0
%assign i 32
%rep 224
    dq interrupt_handler_%+i
%assign i i + 1
%endrep
