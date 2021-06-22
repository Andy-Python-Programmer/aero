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
use alloc::sync::Arc;

use core::alloc::Layout;
use core::ptr::Unique;
use core::sync::atomic::{AtomicUsize, Ordering};

use spin::mutex::spin::SpinMutex;

use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::*;
use x86_64::VirtAddr;

use xmas_elf::program::Type;
use xmas_elf::{header, program, ElfFile};

use crate::arch::interrupts::IretRegisters;
use crate::fs::file_table::FileTable;

use crate::mem::paging::FRAME_ALLOCATOR;
use crate::mem::AddressSpace;

use crate::syscall::SyscallFrame;
use crate::utils::stack::{Stack, StackHelper};

use crate::prelude::*;

extern "C" {
    /// This function is responsible for switching from the provided previous context to
    /// the new one and also save the current state in the previous context so there is a restore
    /// point.
    ///
    /// Check out the documentation of this function in `threading.S` for more information.
    pub(super) fn context_switch(previous: &mut Unique<InterruptFrame>, new: &InterruptFrame);

    /// This function is responsible for stashing the kernel stack and switching to the process stack,
    /// and then jumping to userland.
    ///
    /// Check out the documentation of this function in `threading.S` for more information.
    fn sysretq_userinit();

    /// This function is responsible for switching to the kernel process stack and switching to the kernek
    /// process.
    ///
    /// Check out the documentation of this function in `threading.S` for more information.
    fn iretq_kernelinit();
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ProcessId(usize);

impl ProcessId {
    #[inline(always)]
    pub(super) const fn new(pid: usize) -> Self {
        Self(pid)
    }

    /// Allocates a new process ID. The caller has to garuntee that
    /// the scheduler is locked until you register the process.
    fn allocate() -> Self {
        static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

        Self::new(NEXT_PID.fetch_add(1, Ordering::AcqRel))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProcessState {
    Running,
}

#[repr(C)]
pub(super) struct InterruptFrame {
    pub cr3: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rbx: u64,
    pub rflags: u64,
    pub rip: u64,
}

impl InterruptFrame {
    pub fn new() -> Self {
        Self {
            cr3: 0x00,
            rflags: 0x00,
            r15: 0x00,
            r14: 0x00,
            r13: 0x00,
            r12: 0x00,
            rbp: 0x00,
            rbx: 0x00,
            rip: 0x00,
        }
    }
}

#[repr(C, packed)]
struct KernelProcessFrame {
    pub rdi: usize,
    pub iretq: IretRegisters,
    pub on_finish: usize,
}

pub struct Process {
    pub(super) context: Unique<InterruptFrame>,
    pub(super) address_space: Option<AddressSpace>,
    pub(super) process_id: ProcessId,
    pub(super) context_switch_rsp: VirtAddr,

    pub file_table: FileTable,
    pub state: ProcessState,
}

impl Process {
    /// Creates a per-cpu idle process. An idle process is a special *kernel*
    /// which is executed when there are no runnable processes in the scheduler's
    /// queue.
    pub fn new_idle() -> Arc<SpinMutex<Process>> {
        Arc::new(SpinMutex::new(Self {
            context: Unique::dangling(),
            file_table: FileTable::new(),
            process_id: ProcessId::allocate(),
            context_switch_rsp: VirtAddr::zero(),
            address_space: None,
            state: ProcessState::Running,
        }))
    }

    /// Allocates a new kernel process pointing at the provided entry point address. This function
    /// is responsible for creating the kernel process and setting up the context switch stack itself.
    ///
    /// ## Transition
    /// Userland process transition is done through `iretq` method.
    #[allow(unused)] // FIXME: Use this beautiful code :D
    pub fn new_kernel(entry_point: VirtAddr) -> Arc<SpinMutex<Self>> {
        let process_stack = unsafe {
            let layout = Layout::from_size_align_unchecked(0x1000, 0x100);
            let raw = alloc_zeroed(layout);

            raw
        };

        let kernel_cr3: u64;

        unsafe {
            asm!("mov {}, cr3", out(reg) kernel_cr3, options(nomem));
        }

        /*
         * Now at this stage, we have mapped the kernek process stack. Now we have to allocate a 16KiB stack
         * for the context switch function on the kernel's heap (which should enough) and create the context
         * switch context itself. This includes the syscall and interrupt contexts.
         */
        let mut context_switch_rsp = unsafe {
            let layout = Layout::from_size_align_unchecked(0x400, 0x100);
            let raw = alloc_zeroed(layout);

            raw as u64 + layout.size() as u64
        };

        let mut context_switch = StackHelper::new(&mut context_switch_rsp);
        let kprocess_stack = unsafe { context_switch.offset::<KernelProcessFrame>() };

        kprocess_stack.iretq.rsp = process_stack as u64 - 0x08;
        kprocess_stack.iretq.rip = entry_point.as_u64();
        kprocess_stack.iretq.cs = 0x08; // Ring 0 CS
        kprocess_stack.iretq.ss = 0x10; // Ring 0 DS
        kprocess_stack.iretq.rflags = 1 << 9; // Interrupts enabled.
        kprocess_stack.rdi = 0x00;
        kprocess_stack.on_finish = 0xcafebabe;

        let interrupt_stack = unsafe { context_switch.offset::<InterruptFrame>() };
        *interrupt_stack = InterruptFrame::new(); // Sanitize the interrupt stack.

        interrupt_stack.rip = iretq_kernelinit as u64;
        interrupt_stack.cr3 = kernel_cr3;

        let interrupt_stack =
            unsafe { Unique::new_unchecked(interrupt_stack as *mut InterruptFrame) };

        let context_switch_rsp = unsafe { VirtAddr::new_unsafe(context_switch_rsp) };

        Arc::new(SpinMutex::new(Self {
            context: interrupt_stack,
            address_space: None,
            process_id: ProcessId::allocate(),
            context_switch_rsp,
            file_table: FileTable::new(),
            state: ProcessState::Running,
        }))
    }

    /// Allocates a new userland process from the provided executable ELF. This function
    /// is responsible for mapping the loadable program headers, allocating the user stack,
    /// creating the file tables, creating the userland address space which contains the userland
    /// page tables and finally setting up the process context.
    ///
    /// ## Transition
    /// Userland process transition is done through `sysretq` method.
    pub fn from_user_elf(
        offset_table: &mut OffsetPageTable,
        elf_binary: &ElfFile,
    ) -> Result<Arc<SpinMutex<Self>>, MapToError<Size4KiB>> {
        let raw_binary = elf_binary.input.as_ptr();

        header::sanity_check(elf_binary).expect("The binary failed the sanity check");

        let address_space = AddressSpace::new()?;

        for header in elf_binary.program_iter() {
            program::sanity_check(header, elf_binary).expect("Failed header sanity check");

            let header_type = header.get_type().expect("Unable to get the header type");
            let header_flags = header.flags();

            if let Type::Load = header_type {
                let page_range = {
                    let start_addr = VirtAddr::new(header.virtual_addr());
                    let end_addr = start_addr + header.mem_size() - 1u64;

                    let start_page: Page = Page::containing_address(start_addr);
                    let end_page = Page::containing_address(end_addr);

                    Page::range_inclusive(start_page, end_page)
                };

                let mut flags = PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE
                    | PageTableFlags::WRITABLE;

                if !header_flags.is_execute() {
                    flags |= PageTableFlags::NO_EXECUTE;
                }

                for page in page_range {
                    let frame = unsafe {
                        FRAME_ALLOCATOR
                            .allocate_frame()
                            .ok_or(MapToError::FrameAllocationFailed)?
                    };

                    unsafe { offset_table.map_to(page, frame, flags, &mut FRAME_ALLOCATOR) }?
                        .flush();
                }

                unsafe {
                    memcpy(
                        header.virtual_addr() as *mut u8,
                        raw_binary.add(header.offset() as usize) as *const u8,
                        header.file_size() as usize,
                    );

                    memset(
                        (header.virtual_addr() + header.file_size()) as *mut u8,
                        0,
                        (header.mem_size() - header.file_size()) as usize,
                    );
                }
            }
        }

        let process_stack = {
            let address = unsafe { VirtAddr::new_unsafe(0x80000000) };

            Stack::new_user_pinned(offset_table, address, 0x10000)?
        };

        let entry_point = VirtAddr::new(elf_binary.header.pt2.entry_point());
        let kernel_cr3: u64; // TODO(Andy-Python-Programmer): Switch to the userspace address space

        unsafe {
            asm!("mov {}, cr3", out(reg) kernel_cr3, options(nomem));
        }

        /*
         * Now at this stage, we have mapped the user stack and the the user land ELF executable itself. Now
         * we have to allocate a 16KiB stack for the context switch function on the kernel's heap
         * (which should enough) and create the context switch context itself. This includes the syscall and
         * interrupt contexts.
         */
        let mut context_switch_rsp = unsafe {
            let layout = Layout::from_size_align_unchecked(0x400, 0x100);
            let raw = alloc_zeroed(layout);

            raw as u64 + layout.size() as u64
        };

        let mut context_switch = StackHelper::new(&mut context_switch_rsp);
        let syscall_stack = unsafe { context_switch.offset::<SyscallFrame>() };

        syscall_stack.rsp = process_stack.stack_top().as_u64();
        syscall_stack.rip = entry_point.as_u64();
        syscall_stack.rflags = 1 << 9; // Interrupts enabled.

        let interrupt_stack = unsafe { context_switch.offset::<InterruptFrame>() };
        *interrupt_stack = InterruptFrame::new(); // Sanitize the interrupt stack.

        interrupt_stack.rip = sysretq_userinit as u64;
        interrupt_stack.cr3 = kernel_cr3;

        let interrupt_stack_ptr =
            unsafe { Unique::new_unchecked(interrupt_stack as *mut InterruptFrame) };

        let context_switch_rsp = unsafe { VirtAddr::new_unsafe(context_switch_rsp) };

        Ok(Arc::new(SpinMutex::new(Self {
            context: interrupt_stack_ptr,
            context_switch_rsp,
            file_table: FileTable::new(),
            process_id: ProcessId::allocate(),
            address_space: Some(address_space),
            state: ProcessState::Running,
        })))
    }
}
