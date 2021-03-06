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

use alloc::alloc::alloc_zeroed;

use aero_syscall::{MMapFlags, MMapProt};
use xmas_elf::{header, ElfFile};

use core::{alloc::Layout, ptr::Unique};

use crate::mem::paging::*;
use crate::syscall::{RegistersFrame, SyscallFrame};
use crate::userland::vm::Vm;
use crate::utils::StackHelper;

use super::controlregs;
use super::gdt::{Ring, TASK_STATE_SEGMENT};
use super::interrupts::IretRegisters;

use crate::mem::AddressSpace;

#[repr(C, packed)]
struct KernelTaskFrame {
    rdi: u64,
    iret: IretRegisters,
}

#[derive(Default)]
#[repr(C, packed)]
struct Context {
    cr3: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rbx: u64,
    rflags: u64,
    rip: u64,
}

pub struct ArchTask {
    context: Unique<Context>,
    address_space: AddressSpace,
    context_switch_rsp: VirtAddr,

    rpl: Ring,
    fs_base: VirtAddr,
}

impl ArchTask {
    pub fn new_idle() -> Self {
        Self {
            context: Unique::dangling(),
            context_switch_rsp: VirtAddr::zero(),

            // Since the IDLE task is a special kernel task, we use the kernel's
            // address space here and we also use the kernel privilage level here.
            address_space: AddressSpace::this(),
            rpl: Ring::Ring0,
            fs_base: VirtAddr::zero(),
        }
    }

    /// Allocates a new kernel task pointing at the provided entry point address. This function
    /// is responsible for creating the kernel task and setting up the context switch stack itself.
    pub fn new_kernel(entry_point: VirtAddr, enable_interrupts: bool) -> Self {
        let task_stack = unsafe {
            let layout = Layout::from_size_align_unchecked(4096 * 16, 0x1000);
            alloc_zeroed(layout).add(layout.size())
        };

        // Get the current active address space as we are making the task for
        // the kernel itself.
        let address_space = AddressSpace::this();

        // Now at this stage, we have mapped the kernel task stack. Now we have to set up the
        // context for the kernel task required for the context switch.
        let mut stack_ptr = task_stack as u64;
        let mut stack = StackHelper::new(&mut stack_ptr);

        let kernel_task_frame = unsafe { stack.offset::<KernelTaskFrame>() };

        kernel_task_frame.iret.ss = 0x10; // kernel stack segment
        kernel_task_frame.iret.cs = 0x08; // kernel code segment
        kernel_task_frame.iret.rip = entry_point.as_u64();
        kernel_task_frame.iret.rsp = unsafe { task_stack.sub(8) as u64 };
        kernel_task_frame.iret.rflags = if enable_interrupts { 0x200 } else { 0x00 };

        extern "C" {
            fn iretq_init();
        }

        let context = unsafe { stack.offset::<Context>() };

        *context = Context::default();
        context.rip = iretq_init as u64;
        context.cr3 = controlregs::read_cr3_raw();

        Self {
            context: unsafe { Unique::new_unchecked(context) },
            address_space,
            context_switch_rsp: VirtAddr::new(task_stack as u64),

            // Since we are creating a kernel task, we set the ring privilage
            // level to ring 0.
            rpl: Ring::Ring0,
            fs_base: VirtAddr::zero(),
        }
    }

    pub fn fork(&self) -> Result<Self, MapToError<Size4KiB>> {
        let new_address_space = AddressSpace::this().offset_page_table().fork()?;

        // Since the fork function marks all of the userspace entries in both the forked
        // and the parent address spaces as read only, we will flush the page table of the
        // current process to trigger COW.
        unsafe {
            asm!("mov cr3, {}", in(reg) controlregs::read_cr3_raw(), options(nostack));
        }

        let switch_stack = unsafe {
            let layout = Layout::from_size_align_unchecked(0x1000, 0x1000);
            alloc_zeroed(layout).add(layout.size())
        };

        let mut old_stack_ptr = self.context_switch_rsp.as_u64();
        let mut old_stack = StackHelper::new(&mut old_stack_ptr);

        let mut new_stack_ptr = switch_stack as u64;
        let mut new_stack = StackHelper::new(&mut new_stack_ptr);

        unsafe {
            // Get the syscall frame and registers frame from the current task and copy it over
            // to the fork task.
            *new_stack.offset::<SyscallFrame>() = *old_stack.offset::<SyscallFrame>();

            let registers_frame = new_stack.offset::<RegistersFrame>();
            let old_registers_frame = old_stack.offset::<RegistersFrame>();

            *registers_frame = *old_registers_frame;
            registers_frame.rax = 0x00; // Set the syscall result to 0
        }

        // Prepare the trampoline...
        let context = unsafe { new_stack.offset::<Context>() };

        extern "C" {
            fn sysret_fork_init();
        }

        *context = Context::default();
        context.rip = sysret_fork_init as u64;
        context.cr3 = new_address_space.cr3().start_address().as_u64();

        Ok(Self {
            context: unsafe { Unique::new_unchecked(context) },
            context_switch_rsp: VirtAddr::new(switch_stack as u64),
            address_space: new_address_space,
            rpl: Ring::Ring3,
            fs_base: VirtAddr::zero(),
        })
    }

    pub fn exec(&mut self, vm: &Vm, executable: &ElfFile) -> Result<(), MapToError<Size4KiB>> {
        header::sanity_check(executable).expect("Failed sanity check"); // Sanity check the provided ELF executable

        let address_space = if self.rpl == Ring::Ring0 {
            // If the kernel task wants to execute an executable, then we have to
            // create a new address space for it as we cannot use the kernel's address space
            // here.
            AddressSpace::new()?
        } else {
            // If we are the user who wants to execute an executable, we can just use the
            // current address space allocated for the user and deallocate all of the user
            // page entries.
            //
            // TODO: deallocate the user address space's page entries.
            AddressSpace::this()
        };

        // mmap the userland stack...
        vm.mmap(
            VirtAddr::new(0x8000_0000_0000 - 0x64000),
            0x64000,
            MMapProt::PROT_WRITE | MMapProt::PROT_READ,
            MMapFlags::MAP_FIXED | MMapFlags::MAP_PRIVATE | MMapFlags::MAP_ANONYOMUS,
        );

        vm.log();

        address_space.switch(); // Perform the address space switch

        self.context = Unique::dangling();
        self.address_space = address_space; // Update the address space reference

        extern "C" {
            fn jump_userland_exec(stack: VirtAddr, rip: VirtAddr, rflags: u64);
        }

        let entry_point = VirtAddr::new(executable.header.pt2.entry_point());

        unsafe {
            jump_userland_exec(VirtAddr::new(0x8000_0000_0000), entry_point, 0x200);
        }

        Ok(())
    }

    #[inline]
    pub fn get_fs_base(&mut self) -> VirtAddr {
        self.fs_base
    }

    #[inline]
    pub fn set_fs_base(&mut self, base: VirtAddr) {
        self.fs_base = base;
    }
}

/// This function is responsible for performing the inner task switch. Firstly it sets the
/// new RSP in the TSS and then performes the actual context switch (saving the previous tasks
/// state in its context and then switching to the new task).
pub fn arch_task_spinup(from: &mut ArchTask, to: &ArchTask) {
    extern "C" {
        fn task_spinup(from: &mut Unique<Context>, to: &Context);
    }

    unsafe {
        // Set the stack pointer in the TSS.
        TASK_STATE_SEGMENT.rsp[0] = to.context_switch_rsp.as_u64();

        task_spinup(&mut from.context, to.context.as_ref());
    }
}
