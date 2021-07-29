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

global restore_kernel_fs_base
global restore_kernel_fs_base_locked

; This function is responsible for restoring the kernel thread local
; storage. Note that this function must be invoked under locked conditions.
restore_kernel_fs_base_locked:
    push rbx
    push rdx
    push rcx
    push rax

    mov rbx, qword [gs:104] ; Offset into TSS holding FS_BASE for this cpu
    mov ecx, 0xC0000100     ; IA32_FS_BASE msr
    mov eax, ebx
    shr rbx, 32
    mov edx, ebx

    wrmsr

    pop rax
    pop rcx
    pop rdx
    pop rbx

    ret

; Helper function that calls the `restore_kernel_fs_base_locked` function under
; locked conditions.
restore_kernel_fs_base:
    cli

    pushfq
    swapgs

    call restore_kernel_fs_base_locked

    swapgs
    popfq

    ret
