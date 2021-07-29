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

use aero_syscall::{MMapFlags, MMapProt};
use alloc::alloc::alloc_zeroed;
use xmas_elf::{header, ElfFile};

use core::alloc::Layout;

use crate::{mem::paging::*, userland::vm::Vm};

use super::gdt::{Ring, TASK_STATE_SEGMENT};

use crate::mem::AddressSpace;

#[repr(C)]
#[derive(Default)]
struct Context {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9: u64,
    r8: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
    isr_number: u64,
    err_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

pub struct ArchTask {
    context: Context,
    address_space: AddressSpace,
    context_switch_rsp: VirtAddr,

    rpl: Ring,
}

impl ArchTask {
    pub fn new_idle() -> Self {
        Self {
            context: Context::default(),
            context_switch_rsp: VirtAddr::zero(),

            // Since the IDLE task is a special kernel task, we use the kernel's
            // address space here and we also use the kernel privilage level here.
            address_space: AddressSpace::this(),
            rpl: Ring::Ring0,
        }
    }

    /// Allocates a new kernel task pointing at the provided entry point address. This function
    /// is responsible for creating the kernel task and setting up the context switch stack itself.
    pub fn new_kernel(entry_point: VirtAddr) -> Self {
        let task_stack = unsafe {
            // We want the task stack to be page aligned.
            let layout = Layout::from_size_align_unchecked(0x1000, 0x100);
            let raw = alloc_zeroed(layout);

            raw
        };

        // Get the current active address space as we are making the task for
        // the kernel itself.
        let address_space = AddressSpace::this();

        /*
         * Now at this stage, we have mapped the kernel task stack. Now we have to set up the
         * context for the kernel task required for the context switch.
         */
        let context = Context {
            r15: 0x00,
            r14: 0x00,
            r13: 0x00,
            r12: 0x00,
            r11: 0x00,
            r10: 0x00,
            r9: 0x00,
            r8: 0x00,
            rbp: 0x00,
            rdi: 0x00,
            rsi: 0x00,
            rdx: 0x00,
            rcx: 0x00,
            rbx: 0x00,
            rax: 0x00,
            isr_number: 0x00,
            err_code: 0x00,

            rip: entry_point.as_u64(), // Set the instruction pointer to the entry point.
            cs: 0x08,                  // Kernel code segment.
            rflags: 0x202,             // Interrupts enabled.
            rsp: task_stack as u64,    // Set the stack pointer to the task stack.
            ss: 0x10,                  // Kernel stack segment.
        };

        Self {
            context,
            address_space,
            context_switch_rsp: VirtAddr::new(task_stack as u64),

            // Since we are creating a kernel task, we set the ring privilage
            // level to ring 0.
            rpl: Ring::Ring0,
        }
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
            MMapProt::PROT_WRITE | MMapProt::PROT_READ | MMapProt::PROT_EXEC,
            MMapFlags::MAP_FIXED | MMapFlags::MAP_PRIVATE | MMapFlags::MAP_ANONYOMUS,
        );

        vm.log();

        address_space.switch(); // Perform the address space switch

        self.context = Context::default();
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
}

/// This function is responsible for performing the inner task switch. Firstly it sets the
/// new RSP in the TSS and then performes the actual context switch (saving the previous tasks
/// state in its context and then switching to the new task).
pub fn arch_task_spinup(to: &mut ArchTask, address_space_switch: bool) {
    extern "C" {
        fn task_spinup(context: u64, cr3: u64);
    }

    unsafe {
        // Set the stack pointer in the TSS.
        TASK_STATE_SEGMENT.rsp[0] = to.context_switch_rsp.as_u64();

        if address_space_switch {
            let cr3 = to.address_space.cr3().start_address().as_u64();

            // Perform the context switch with the new address space.
            task_spinup((&mut to.context as *mut Context) as u64, cr3)
        } else {
            // Perform the context switch without switching the address space.
            task_spinup((&mut to.context as *mut Context) as u64, 0x00)
        }
    }
}
