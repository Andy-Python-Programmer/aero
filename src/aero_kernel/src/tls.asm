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
