; Copyright 2021 The Aero Project Developers. See the COPYRIGHT
; file at the top-level directory of this project.
;
; Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
; http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
; <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
; option. This file may not be copied, modified, or distributed
; except according to those terms.

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;; BITS 16 ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
org 0x8000
section .text

[bits 16]

trampoline:
    jmp short protected_ap
    times 8 - ($ - trampoline) nop

    .ready: dq 0
    .cpu_id: dq 0
    .page_table: dq 0
    .stack_start: dq 0
    .stack_end: dq 0
    .code: dq 0

protected_ap:
    cli

    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax

    ; Initialize stack to null
    mov sp, 0x00

    mov edi, trampoline.page_table
    mov cr3, edi

    ; Enable FPU
    mov eax, cr0
    and al, 11110011b
    or al, 00100010b
    mov cr0, eax

    mov eax, cr4
    or eax, 1 << 18 | 1 << 10 | 1 << 9 | 1 << 7 | 1 << 5 | 1 << 4
    mov cr4, eax

    fninit

    ; Load protected mode GDT.
    lgdt [gdtr]

    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x100
    wrmsr

    ; Enable paging and protection.
    ;
    ; 31: Paging
    ; 16: Write protect
    ; 0: Protected Mode
    mov eax, cr0

    or eax, 1 << 31
    or eax, 1 << 16
    or eax, 1

    mov cr0, eax

    ; Enable long mode and load CS with 64 bit segment.
    jmp gdt.kernel_code:long_mode_ap

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;; BITS 64 ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
[bits 64]

long_mode_ap:
    mov rax, gdt.kernel_data
    mov ds, rax
    mov es, rax
    mov fs, rax
    mov gs, rax
    mov ss, rax

    mov rcx, [trampoline.stack_end]
    lea rsp, [rcx - 256]

    mov rdi, trampoline.cpu_id

    mov rax, [trampoline.code]
    mov qword [trampoline.ready], 1
    jmp rax

struc GDTEntry
    .limitl resw 1
    .basel resw 1
    .basem resb 1
    .attribute resb 1
    .flags__limith resb 1
    .baseh resb 1
endstruc

attrib:
    .present              equ 1 << 7
    .ring1                equ 1 << 5
    .ring2                equ 1 << 6
    .ring3                equ 1 << 5 | 1 << 6
    .user                 equ 1 << 4
    .code                 equ 1 << 3
    .conforming           equ 1 << 2
    .readable             equ 1 << 1
    .expand_down          equ 1 << 2
    .writable             equ 1 << 1
    .accessed             equ 1 << 0
    .tssAvailabe16        equ 0x1
    .ldt                  equ 0x2
    .tssBusy16            equ 0x3
    .call16               equ 0x4
    .task                 equ 0x5
    .interrupt16          equ 0x6
    .trap16               equ 0x7
    .tssAvailabe32        equ 0x9
    .tssBusy32            equ 0xB
    .call32               equ 0xC
    .interrupt32          equ 0xE
    .trap32               equ 0xF
    .ldt32                equ 0x2
    .tssAvailabe64        equ 0x9
    .tssBusy64            equ 0xB
    .call64               equ 0xC
    .interrupt64          equ 0xE
    .trap64               equ 0xF

flags:
    .granularity equ 1 << 7
    .available equ 1 << 4
    .default_operand_size equ 1 << 6
    .long_mode equ 1 << 5
    .reserved equ 1 << 5

gdtr:
    dw gdt.end + 1
    dq gdt

gdt:
.null equ $ - gdt
    dq 0

.kernel_code equ $ - gdt
istruc GDTEntry
    at GDTEntry.limitl, dw 0
    at GDTEntry.basel, dw 0
    at GDTEntry.basem, db 0
    at GDTEntry.attribute, db attrib.present | attrib.user | attrib.code
    at GDTEntry.flags__limith, db flags.long_mode
    at GDTEntry.baseh, db 0
iend

.kernel_data equ $ - gdt
istruc GDTEntry
    at GDTEntry.limitl, dw 0
    at GDTEntry.basel, dw 0
    at GDTEntry.basem, db 0
    at GDTEntry.attribute, db attrib.present | attrib.user | attrib.writable
    at GDTEntry.flags__limith, db 0
    at GDTEntry.baseh, db 0
iend

.end equ $ - gdt