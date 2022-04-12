/*
 * Copyright (C) 2021-2022 The Aero Project Developers.
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

//! ## How does `x86_64` context switching work?
//!
//! The [`arch_task_spinup`] function is responsible for switching the current
//! task to the next one. This function works by updating the TSS's RSP0 field to point
//! to the per-task kernel stack and switches the page table for the next process.
//!
//! After a task is born, it directly context switches to it's specific trampoline. The
//! trampoline is responsible for jumping to its appropriate context. At the point when
//! the trampoline is called, the stack should have the switch frame pushed.
//!
//! The switch frame is not saved or restored at context switch time. Instead its
//! stored when you enter and leave the kernel since then the context switch function
//! does not have to worry about clobbering the user mode register values since
//! they are safely stored on the kernel stack.

use alloc::alloc::alloc_zeroed;

use aero_syscall::{MMapFlags, MMapProt};
use alloc::vec::Vec;

use core::alloc::Layout;
use core::ptr::Unique;

use crate::arch::interrupts::InterruptErrorStack;
use crate::fs::cache::DirCacheItem;
use crate::mem::paging::*;
use crate::syscall::ExecArgs;
use crate::userland::vm::Vm;
use crate::utils::{io, StackHelper};

use super::controlregs;

use crate::mem::AddressSpace;

#[derive(Default)]
#[repr(C)]
struct Context {
    cr3: u64,

    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,

    rbx: u64,
    rbp: u64,

    rip: u64,
}

#[repr(u64)]
#[derive(Debug, Copy, Clone)]
pub enum AuxvType {
    AtNull = 0,
    AtPhdr = 3,
    AtPhEnt = 4,
    AtPhNum = 5,
    AtEntry = 9,
}

/// Returns the first address outside the user range.
///
/// ## Notes
/// * On Intel CPUs, if a SYSCALL instruction is at the highest canonical address, then
/// that syscall will enter the kernel with a non-canonical return address, and SYSRET will
/// explode dangerously. We avoid this particular problem by preventing anything from
/// being mapped at the maximum canonical address.
///
/// * On AMD CPUs in the Ryzen family, there's a nasty bug in which the CPUs malfunction if they
/// execute code from the highest canonical page. They'll speculate right off the end of the canonical
/// space and bad things happen. This is worked around in the same way as the Intel problem.
pub fn userland_last_address() -> VirtAddr {
    // Reference: https://elixir.bootlin.com/linux/latest/source/arch/x86/include/asm/page_64.h#L61
    static CACHED: spin::Once<VirtAddr> = spin::Once::new();

    *CACHED.call_once(|| {
        let virtual_mask_shift: u64;
        let la57 = crate::mem::paging::level_5_paging_enabled();

        if la57 {
            virtual_mask_shift = 56;
        } else {
            virtual_mask_shift = 47;
        }

        VirtAddr::new((1u64 << virtual_mask_shift) - Size4KiB::SIZE)
    })
}

const USERLAND_STACK_SIZE: u64 = 0x64000;

//(1 << 47) - (Size4KiB::SIZE * 2)
const USERLAND_STACK_TOP: VirtAddr = VirtAddr::new(0x7fffffffe000);
const USERLAND_STACK_BOTTOM: VirtAddr = USERLAND_STACK_TOP.const_sub_u64(USERLAND_STACK_SIZE);

pub struct ArchTask {
    context: Unique<Context>,

    address_space: AddressSpace,
    context_switch_rsp: VirtAddr,
    user: bool,

    fs_base: VirtAddr,
    gs_base: VirtAddr,
}

impl ArchTask {
    pub fn new_idle() -> Self {
        Self {
            context: Unique::dangling(),
            context_switch_rsp: VirtAddr::zero(),

            // Since the IDLE task is a special kernel task, we use the kernel's
            // address space here and we also use the kernel privilage level here.
            address_space: AddressSpace::this(),
            user: false,

            fs_base: VirtAddr::zero(),
            gs_base: VirtAddr::zero(),
        }
    }

    pub fn new_kernel(entry_point: VirtAddr, enable_interrupts: bool) -> Self {
        let switch_stack = Self::alloc_switch_stack().unwrap().as_mut_ptr::<u8>();

        let task_stack = unsafe {
            let layout = Layout::from_size_align_unchecked(4096 * 16, 0x1000);
            alloc_zeroed(layout).add(layout.size())
        };

        let address_space = AddressSpace::this();

        let mut stack_ptr = switch_stack as u64;
        let mut stack = StackHelper::new(&mut stack_ptr);

        let kframe = unsafe { stack.offset::<InterruptErrorStack>() };

        kframe.stack.iret.ss = 0x10; // kernel stack segment
        kframe.stack.iret.cs = 0x08; // kernel code segment
        kframe.stack.iret.rip = entry_point.as_u64();
        kframe.stack.iret.rsp = task_stack as u64;
        kframe.stack.iret.rflags = if enable_interrupts { 0x200 } else { 0x00 };

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
            context_switch_rsp: VirtAddr::new(switch_stack as u64),
            user: false,

            fs_base: VirtAddr::zero(),
            gs_base: VirtAddr::zero(),
        }
    }

    pub fn clone_process(
        &self,
        _entry: usize,
        _stack: usize,
    ) -> Result<Self, MapToError<Size4KiB>> {
        todo!("implement clone_process")
    }

    pub fn fork(&self) -> Result<Self, MapToError<Size4KiB>> {
        assert!(self.user, "cannot fork a kernel task");

        let new_address_space = AddressSpace::this().offset_page_table().fork()?;

        // Since the fork function marks all of the userspace entries in both the forked
        // and the parent address spaces as read only, we will flush the page table of the
        // current process to trigger COW.
        unsafe {
            asm!("mov cr3, {}", in(reg) controlregs::read_cr3_raw(), options(nostack));
        }

        let switch_stack = Self::alloc_switch_stack()?.as_mut_ptr::<u8>();

        let mut old_stack_ptr = self.context_switch_rsp.as_u64();
        let mut old_stack = StackHelper::new(&mut old_stack_ptr);

        let mut new_stack_ptr = switch_stack as u64;
        let mut new_stack = StackHelper::new(&mut new_stack_ptr);

        unsafe {
            let registers_frame = new_stack.offset::<InterruptErrorStack>();
            let old_registers_frame = old_stack.offset::<InterruptErrorStack>();

            *registers_frame = *old_registers_frame;
            registers_frame.stack.scratch.rax = 0x00; // Set the syscall result to 0
        }

        // Prepare the trampoline...
        let context = unsafe { new_stack.offset::<Context>() };

        extern "C" {
            fn fork_init();
        }

        *context = Context::default();
        context.rip = fork_init as u64;
        context.cr3 = new_address_space.cr3().start_address().as_u64();

        Ok(Self {
            context: unsafe { Unique::new_unchecked(context) },
            context_switch_rsp: VirtAddr::new(switch_stack as u64),
            address_space: new_address_space,
            user: false,

            // The FS and GS bases are inherited from the parent process.
            fs_base: self.fs_base.clone(),
            gs_base: self.gs_base.clone(),
        })
    }

    pub fn exec(
        &mut self,
        vm: &Vm,
        executable: DirCacheItem,

        argv: Option<ExecArgs>,
        envv: Option<ExecArgs>,
    ) -> Result<(), MapToError<Size4KiB>> {
        let address_space = if self.user {
            self.unref_pt();
            AddressSpace::new()?
        } else {
            AddressSpace::new()?
        };

        let loaded_binary = vm
            .load_bin(executable, argv, envv)
            .expect("exec: failed to load ELF");

        // a kernel task can only execute a user executable
        self.user = true;

        // mmap the userland stack...
        vm.mmap(
            USERLAND_STACK_BOTTOM,
            USERLAND_STACK_SIZE as usize,
            MMapProt::PROT_WRITE | MMapProt::PROT_READ,
            MMapFlags::MAP_FIXED | MMapFlags::MAP_PRIVATE | MMapFlags::MAP_ANONYOMUS,
            0,
            None,
        );

        vm.log();

        address_space.switch(); // Perform the address space switch

        self.context = Unique::dangling();
        self.address_space = address_space; // Update the address space reference

        self.fs_base = VirtAddr::zero();

        extern "C" {
            fn jump_userland_exec(stack: VirtAddr, rip: VirtAddr, rflags: u64);
        }

        let mut stack_addr = USERLAND_STACK_TOP.as_u64();
        let mut stack = StackHelper::new(&mut stack_addr);

        let mut envp = Vec::new();
        let mut argp = Vec::new();

        loaded_binary
            .envv
            .map(|envv| envp = envv.push_into_stack(&mut stack));

        loaded_binary
            .argv
            .map(|argv| argp = argv.push_into_stack(&mut stack));

        stack.align_down();

        let size = envp.len() + 1 + argp.len() + 1 + 1;

        if size % 2 == 1 {
            unsafe {
                stack.write(0u64);
            }
        }

        let p2_header = loaded_binary.elf.header.pt2;

        unsafe {
            let hdr: [(AuxvType, usize); 4] = [
                (
                    AuxvType::AtPhdr,
                    (p2_header.ph_offset() + loaded_binary.base_addr.as_u64()) as usize,
                ),
                (AuxvType::AtPhEnt, p2_header.ph_entry_size() as usize),
                (AuxvType::AtPhNum, p2_header.ph_count() as usize),
                (AuxvType::AtEntry, p2_header.entry_point() as usize),
            ];

            stack.write(0usize); // Make it 16 bytes aligned
            stack.write(AuxvType::AtNull);
            stack.write(hdr);
        }

        // struct ExecStackData {
        //     argc: isize,
        //     argv: *const *const u8,
        //     envv: *const *const u8,
        // }
        unsafe {
            stack.write(0u64);
            stack.write_slice(envp.as_slice());
            stack.write(0u64);
            stack.write_slice(argp.as_slice());
            stack.write(argp.len());
        }

        core::mem::drop(envp);
        core::mem::drop(argp);

        assert_eq!(stack.top() % 16, 0);

        unsafe {
            jump_userland_exec(VirtAddr::new(stack.top()), loaded_binary.entry_point, 0x200);
        }

        Ok(())
    }

    /// Allocates a new context switch stack for the process and returns the stack
    /// top address. See the module level documentation for more information.
    fn alloc_switch_stack() -> Result<VirtAddr, MapToError<Size4KiB>> {
        let frame: PhysFrame<Size4KiB> = FRAME_ALLOCATOR
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        Ok(frame.start_address().as_hhdm_virt() + Size4KiB::SIZE)
    }

    fn unref_pt(&mut self) {
        self.address_space
            .offset_page_table()
            .page_table()
            .for_each_entry(
                PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE,
                |entry| {
                    entry.unref_vm_frame();
                    entry.set_unused();

                    Ok(())
                },
            )
            .expect("dealloc: failed to unref the page table");
    }

    /// Deallocates the architecture-specific task resources. This function is called
    /// when the process is turned into a zombie.
    pub fn dealloc(&mut self) {
        if self.user {
            self.unref_pt();
        }

        // deallocate the switch stack
        {
            let frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(
                (self.context_switch_rsp - Size4KiB::SIZE).as_hhdm_phys(),
            );

            FRAME_ALLOCATOR.deallocate_frame(frame);
        }
    }

    /// Returns the saved GS base for this task.
    pub fn get_gs_base(&self) -> VirtAddr {
        self.gs_base
    }

    /// Sets the GS base to the provided `base`.
    ///
    /// ## Safety
    /// This function **must** be called by the process that this [`ArchTask`] instance
    /// belongs to. This is required since we also update the GS base register with the
    /// `base` immediately (not waiting for a switch).
    pub unsafe fn set_gs_base(&mut self, base: VirtAddr) {
        io::wrmsr(io::IA32_KERNEL_GSBASE, base.as_u64());
        self.gs_base = base;
    }

    /// Returns the saved FS base for this task.
    pub fn get_fs_base(&self) -> VirtAddr {
        self.fs_base
    }

    /// Sets the FS base to the provided `base`.
    ///
    /// ## Safety
    /// This function **must** be called by the process that this [`ArchTask`] instance
    /// belongs to. This is required since we also update the FS base register with the
    /// `base` immediately (not waiting for a switch).
    pub unsafe fn set_fs_base(&mut self, base: VirtAddr) {
        io::wrmsr(io::IA32_FS_BASE, base.as_u64());
        self.fs_base = base;
    }
}

/// Check out the module level documentation for more information.
pub fn arch_task_spinup(from: &mut ArchTask, to: &ArchTask) {
    extern "C" {
        fn task_spinup(from: &mut Unique<Context>, to: &Context);
    }

    unsafe {
        // Load the new thread's kernel stack pointer everywhere it's needed.
        let kstackp = to.context_switch_rsp.as_u64();
        super::gdt::get_task_state_segement().rsp[0] = kstackp;
        io::wrmsr(io::IA32_SYSENTER_ESP, kstackp);

        task_spinup(&mut from.context, to.context.as_ref());

        // make a restore point for the current FS base.
        from.fs_base = VirtAddr::new(io::rdmsr(io::IA32_FS_BASE));
        // switch to the new FS base.
        io::wrmsr(io::IA32_FS_BASE, to.fs_base.as_u64());

        // make a restore point for the current GS base.
        from.gs_base = VirtAddr::new(io::rdmsr(io::IA32_KERNEL_GSBASE));
        // update the swap GS target to point to the new GS base.
        io::wrmsr(io::IA32_KERNEL_GSBASE, to.gs_base.as_u64());
    }
}
