/*
 * Copyright (C) 2021 The Aero Project Developers.
 *
 * This file is part of The Aero Project.
 *
 * Aero is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Aero is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Aero. If not, see <https://www.gnu.org/licenses/>.
 */

/*
 * This function is responsible for switching from the provided previous context to
 * the new one and also save the current state in the previous context so there is a restore
 * point (explains the &mut reference requirement).
 *
 * ## Signature
 * ```rust
 * extern "C" fn context_switch(previous: &mut Unique<InterruptFrame>, new: &InterruptFrame);
 * ```
 */
context_switch:
    pushfq /* Push registers to current context. */

    cli /* Disable interrupts as we are switching stacks. */

    push rbp
    push r15
    push r14
    push r13
    push r12
    push rbx

    mov rax, cr3 /* Save CR3. */
    push rax

    mov [rdi], rsp /* Update old context pointer with current stack pointer. */
    mov rsp, rsi /* Switch to new stack. */

    pop rax /* Restore CR3. */
    mov cr3, rax

    pop rbx
    pop r12
    pop r13
    pop r14
    pop r15
    pop rbp

    popfq

    ret

.global context_switch

/*
 * This function is responsible for stashing the kernel stack and switching to the process stack,
 * and then jumping to userland.
 *
 * ## Signature
 * ```rust
 * extern "C" fn sysretq_userinit();
 * ```
 */
sysretq_userinit:
    /*
     * After pushing all of the required registers on the stack
     * disable interrupts as we are swaping stacks. Interrupts are
     * automatically enabled after `sysretq`.
     */
    cli
    call context_switch_finalize
    call restore_user_tls

    pop r11 /* Restore RFLAGS. */
    pop rcx /* Restore RIP. */

    push rdx

    swapgs

    mov rdx, rsp
    add rdx, 16 // Skip RDX and user RSP currently on the stack.
    mov gs:[0x04], rdx // Stash kernel stack.

    swapgs
    pop rdx
    pop rsp /* Restore user stack. */

    sysretq

.global sysretq_userinit

iretq_kernelinit:
    pop rdi
    iretq // Leap of faith!

.global iretq_kernelinit

/*
 * This function is responsible for jumping to userland with the provided stack, instruction pointer
 * and argument. This is used by sys_exec(2).
 *
 * ## Signature
 * ```rust
 * pub extern "C" fn jump_userland(stack_top: VirtAddr, instruction_ptr: VirtAddr, argument: u64);
 * ```
 *
 * ## Saftey
 * Here its is *safe* to use [VirtAddr] as the argument type as it is represented as a
 * transparent struct. So after compilation the argument should result in u64 instead
 * of [VirtAddr].
 */
jump_userland:
    /*
     * After pushing all of the required registers on the stack
     * disable interrupts as we are swaping stacks. Interrupts are
     * automatically enabled after `sysretq`.
     */
    cli

    push rdi // Param: stack_top
    push rsi // Param: instruction_ptr
    push rdx // Param: rflags

    mov ax, (6 << 3) | 3 // Set AX to user data segment with RPL 3

    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    /* NOTE: We do not need to set SS as is handled by `sysretq.` */

    call restore_user_tls

    pop r11
    pop rcx
    pop rsp

    fninit
    sysretq

.global jump_userland
