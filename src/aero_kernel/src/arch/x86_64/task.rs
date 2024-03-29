// Copyright (C) 2021-2024 The Aero Project Developers.
//
// This file is part of The Aero Project.
//
// Aero is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Aero is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Aero. If not, see <https://www.gnu.org/licenses/>.

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
use raw_cpuid::CpuId;

use core::alloc::Layout;
use core::ptr::Unique;

use crate::arch::interrupts::InterruptErrorStack;
use crate::fs::cache::DirCacheItem;
use crate::mem::paging::*;
use crate::syscall::ExecArgs;
use crate::userland::vm::Vm;
use crate::utils::StackHelper;

use super::{asm_macros, controlregs, io};

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
    Null = 0,
    Phdr = 3,
    PhEnt = 4,
    PhNum = 5,
    Entry = 9,
    Secure = 23,
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
/// execute code from the highest canonical page. They'll speculate right off the end of the
/// canonical space and bad things happen. This is worked around in the same way as the Intel
/// problem.
pub fn userland_last_address() -> VirtAddr {
    // Reference: https://elixir.bootlin.com/linux/latest/source/arch/x86/include/asm/page_64.h#L61
    static CACHED: spin::Once<VirtAddr> = spin::Once::new();

    *CACHED.call_once(|| {
        let la57 = crate::mem::paging::level_5_paging_enabled();
        let virtual_mask_shift: u64 = if la57 { 56 } else { 47 };

        VirtAddr::new((1u64 << virtual_mask_shift) - Size4KiB::SIZE)
    })
}

/// Returns whether the given pointer is within the userland address space.
pub fn user_access_ok<T>(ptr: *const T) -> bool {
    let size = core::mem::size_of::<T>();
    VirtAddr::new(ptr as u64 + size as u64) <= super::task::userland_last_address()
}

const USERLAND_STACK_SIZE: u64 = 0x64000;

//(1 << 47) - (Size4KiB::SIZE * 2)
const USERLAND_STACK_TOP: VirtAddr = VirtAddr::new(0x7fffffffe000);
const USERLAND_STACK_BOTTOM: VirtAddr = USERLAND_STACK_TOP.const_sub_u64(USERLAND_STACK_SIZE);

#[naked]
unsafe extern "C" fn jump_userland_exec(stack: VirtAddr, rip: VirtAddr, rflags: u64) {
    asm!(
        "push rdi", // stack
        "push rsi", // rip
        "push rdx", // rflags
        "cli",
        "pop r11",
        "pop rcx",
        "pop rsp",
        "swapgs",
        "sysretq",
        options(noreturn)
    );
}

#[naked]
unsafe extern "C" fn task_spinup(prev: &mut Unique<Context>, next: &Context) {
    asm!(
        // save callee-saved registers
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        // save CR3
        "mov rax, cr3",
        "push rax",
        // update old context
        "mov [rdi], rsp",
        // switch to new stack
        "mov rsp, rsi",
        // restore CR3
        "pop rax",
        "mov cr3, rax",
        // restore callee-saved registers
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        // resume the next thread
        "ret",
        options(noreturn)
    );
}

#[naked]
unsafe extern "C" fn iretq_init() {
    asm!(
        "cli",
        // pop the error code
        "add rsp, 8",
        asm_macros::pop_preserved!(),
        asm_macros::pop_scratch!(),
        "iretq",
        options(noreturn)
    )
}

#[naked]
unsafe extern "C" fn fork_init() {
    asm!(
        "cli",
        // pop the error code
        "add rsp, 8",
        asm_macros::pop_preserved!(),
        asm_macros::pop_scratch!(),
        "swapgs",
        "iretq",
        options(noreturn)
    )
}

pub struct ArchTask {
    context: Unique<Context>,

    address_space: AddressSpace,
    context_switch_rsp: VirtAddr,
    user: bool,

    fs_base: VirtAddr,
    gs_base: VirtAddr,

    pub fpu_storage: Option<FpuState>,
}

impl ArchTask {
    pub fn new_idle() -> Self {
        Self {
            context: Unique::dangling(),
            context_switch_rsp: VirtAddr::zero(),

            // Since the IDLE task is a special kernel task, we use the kernel's
            // address space here and we also use the kernel privilege level here.
            address_space: AddressSpace::this(),
            user: false,

            fs_base: VirtAddr::zero(),
            gs_base: VirtAddr::zero(),

            fpu_storage: None,
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

            fpu_storage: None,
        }
    }

    pub fn clone_process(
        &self,
        entry: usize,
        usr_stack: usize,
    ) -> Result<Self, MapToError<Size4KiB>> {
        log::trace!("ArchTask::clone_process(entry={entry:#x}, stack={usr_stack:#x})");

        assert!(self.user, "cannot clone a kernel task");

        let address_space = AddressSpace::this();
        let switch_stack = Self::alloc_switch_stack()?.as_mut_ptr::<u8>();

        let mut new_stack_ptr = switch_stack as u64;
        let mut new_stack = StackHelper::new(&mut new_stack_ptr);

        let mut old_stack_ptr = self.context_switch_rsp.as_u64();
        let mut old_stack = StackHelper::new(&mut old_stack_ptr);

        let old_registers_frame = unsafe { old_stack.offset::<InterruptErrorStack>() };

        let registers_frame = unsafe { new_stack.offset::<InterruptErrorStack>() };
        *registers_frame = *old_registers_frame;

        // registers_frame.stack.iret.cs = old_registers_frame.stack.iret.cs;
        // registers_frame.stack.iret.ss = old_registers_frame.stack.iret.ss;
        // registers_frame.stack.iret.rflags = old_registers_frame.stack.iret.rflags;

        registers_frame.stack.iret.rip = entry as _;
        registers_frame.stack.iret.rsp = usr_stack as _;
        // registers_frame.stack.iret.rflags = 0x200;

        let context = unsafe { new_stack.offset::<Context>() };

        *context = Context::default();
        context.rip = fork_init as _;
        context.cr3 = address_space.cr3().start_address().as_u64();

        let mut fpu_storage = self.fpu_storage.unwrap().clone();

        Ok(Self {
            context: unsafe { Unique::new_unchecked(context) },
            context_switch_rsp: VirtAddr::new(switch_stack as u64),
            address_space,
            user: true,

            // The FS and GS bases are inherited from the parent process.
            fs_base: VirtAddr::new(1),
            gs_base: self.gs_base,

            fpu_storage: Some(fpu_storage),
        })
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

        *context = Context::default();
        context.rip = fork_init as u64;
        context.cr3 = new_address_space.cr3().start_address().as_u64();

        let fpu_storage = self.fpu_storage.unwrap().clone();

        Ok(Self {
            context: unsafe { Unique::new_unchecked(context) },
            context_switch_rsp: VirtAddr::new(switch_stack as u64),
            address_space: new_address_space,
            user: true,

            // The FS and GS bases are inherited from the parent process.
            fs_base: self.fs_base,
            gs_base: self.gs_base,

            fpu_storage: Some(fpu_storage),
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

        address_space.switch(); // Perform the address space switch

        self.context = Unique::dangling();
        self.address_space = address_space; // Update the address space reference

        self.fs_base = VirtAddr::zero();
        self.gs_base = VirtAddr::zero();

        let mut fpu_storage = FpuState::default();

        // unsafe {
        //     xrstor(&fpu_storage);

        //     // The x87 FPU control word is set to 0x37f (default), which masks all
        //     // floating-point exceptions, sets rounding to nearest, and sets the x87
        //     // FPU precision to 64 bits (as documented in Intel SDM volume 1 section
        //     // 8.1.5).
        //     const DEFAULT_FPU_CWORD: u16 = 0x37f;
        //     asm!("fldcw [{}]", in(reg) &DEFAULT_FPU_CWORD, options(nomem));

        //     // Set the default MXCSR value at reset as documented in Intel SDM volume 2A.
        //     controlregs::write_mxcsr(
        //         MxCsr::INVALID_OPERATION_MASK
        //             | MxCsr::DENORMAL_MASK
        //             | MxCsr::DIVIDE_BY_ZERO_MASK
        //             | MxCsr::OVERFLOW_MASK
        //             | MxCsr::UNDERFLOW_MASK
        //             | MxCsr::PRECISION_MASK,
        //     );

        //     xsave(&mut fpu_storage);
        // }

        self.fpu_storage = Some(fpu_storage);

        let mut stack_addr = USERLAND_STACK_TOP.as_u64();
        let mut stack = StackHelper::new(&mut stack_addr);

        let mut envp = Vec::new();
        let mut argp = Vec::new();

        if let Some(envv) = loaded_binary.envv {
            envp = envv.push_into_stack(&mut stack);
        }

        if let Some(argv) = loaded_binary.argv {
            argp = argv.push_into_stack(&mut stack);
        }

        stack.align_down();

        let size = envp.len() + 1 + argp.len() + 1 + 1;

        if size % 2 == 1 {
            unsafe {
                stack.write(0u64);
            }
        }

        let p2_header = loaded_binary.elf.header.pt2;

        unsafe {
            let hdr: [(AuxvType, usize); 5] = [
                (
                    AuxvType::Phdr,
                    (p2_header.ph_offset() + loaded_binary.base_addr.as_u64()) as usize,
                ),
                (AuxvType::PhEnt, p2_header.ph_entry_size() as usize),
                (AuxvType::PhNum, p2_header.ph_count() as usize),
                (AuxvType::Entry, p2_header.entry_point() as usize),
                (AuxvType::Secure, 0),
            ];

            stack.write(0usize); // Make it 16 bytes aligned
            stack.write(AuxvType::Null);
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
        let frame = FRAME_ALLOCATOR.alloc_zeroed(4096 * 4).unwrap();

        Ok(frame.as_hhdm_virt() + (4096u64 * 4))
    }

    fn unref_pt(&mut self) {
        assert!(self.user);

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
        self.gs_base = base;
        io::set_inactive_gsbase(base);
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
        self.fs_base = base;
        io::set_fsbase(base);
    }
}

fn xsave_size() -> u32 {
    static XSAVE_SIZE: spin::Once<u32> = spin::Once::new();
    *XSAVE_SIZE.call_once(|| {
        CpuId::new()
            .get_extended_state_info()
            .expect("xsave: cpuid extended state info unavailable")
            .xsave_size()
    })
}

#[derive(Debug, Copy, Clone)]
#[repr(C, align(16))]
pub struct FpuState {
    /// x87 FPU Control Word (16 bits). See Figure 8-6 in the Intel® 64 and IA-32 Architectures
    /// Software Developer’s Manual Volume 1, for the layout of the x87 FPU control word.
    pub fcw: u16,
    /// x87 FPU Status Word (16 bits).
    pub fsw: u16,
    /// x87 FPU Tag Word (8 bits) + reserved (8 bits).
    pub ftw: u16,
    /// x87 FPU Opcode (16 bits).
    pub fop: u16,
    /// x87 FPU Instruction Pointer Offset ([31:0]). The contents of this field differ depending on
    /// the current addressing mode (32-bit, 16-bit, or 64-bit) of the processor when the
    /// FXSAVE instruction was executed: 32-bit mode — 32-bit IP offset. 16-bit mode — low 16
    /// bits are IP offset; high 16 bits are reserved. 64-bit mode with REX.W — 64-bit IP
    /// offset. 64-bit mode without REX.W — 32-bit IP offset.
    pub fip: u32,
    /// x87 FPU Instruction Pointer Selector (16 bits) + reserved (16 bits).
    pub fcs: u32,
    /// x87 FPU Instruction Operand (Data) Pointer Offset ([31:0]). The contents of this field
    /// differ depending on the current addressing mode (32-bit, 16-bit, or 64-bit) of the
    /// processor when the FXSAVE instruction was executed: 32-bit mode — 32-bit DP offset.
    /// 16-bit mode — low 16 bits are DP offset; high 16 bits are reserved. 64-bit mode
    /// with REX.W — 64-bit DP offset. 64-bit mode without REX.W — 32-bit DP offset.
    pub fdp: u32,
    /// x87 FPU Instruction Operand (Data) Pointer Selector (16 bits) + reserved.
    pub fds: u32,
    /// MXCSR Register State (32 bits).
    pub mxcsr: u32,
    /// This mask can be used to adjust values written to the MXCSR register, ensuring that
    /// reserved bits are set to 0. Set the mask bits and flags in MXCSR to the mode of
    /// operation desired for SSE and SSE2 SIMD floating-point instructions.
    pub mxcsr_mask: u32,
    /// x87 FPU or MMX technology registers. Layout: [12 .. 9 | 9 ... 0] LHS = reserved; RHS = mm.
    pub mm: [u128; 8],
    /// XMM registers (128 bits per field).
    pub xmm: [u128; 16],
    /// reserved.
    pub _pad: [u64; 12],
}

impl Default for FpuState {
    fn default() -> Self {
        Self {
            mxcsr: 0x1f80,
            mxcsr_mask: 0x037f,
            // rest are zeroed
            fcw: 0,
            fsw: 0,
            ftw: 0,
            fop: 0,
            fip: 0,
            fcs: 0,
            fdp: 0,
            fds: 0,
            mm: [0; 8],
            xmm: [u128::MAX; 16],
            _pad: [0; 12],
        }
    }
}

fn xsave(fpu: &mut FpuState) {
    // The implicit EDX:EAX register pair specifies a 64-bit instruction mask. The specific state
    // components saved correspond to the bits set in the requested-feature bitmap (RFBM), which is
    // the logical-AND of EDX:EAX and XCR0.
    // unsafe {
    //     asm!("xsave64 [{}]", in(reg) fpu.as_ptr(), in("eax") u32::MAX, in("edx") u32::MAX,
    // options(nomem, nostack)) }

    use core::arch::x86_64::_fxsave64;

    unsafe { _fxsave64((fpu as *mut FpuState).cast()) }
}

fn xrstor(fpu: &FpuState) {
    // unsafe {
    //     asm!("xrstor [{}]", in(reg) fpu.as_ptr(), in("eax") u32::MAX, in("edx") u32::MAX,
    // options(nomem, nostack)); }
    use core::arch::x86_64::_fxrstor64;

    unsafe { _fxrstor64((fpu as *const FpuState).cast()) }
}

/// Check out the module level documentation for more information.
pub fn arch_task_spinup(from: &mut ArchTask, to: &ArchTask) {
    unsafe {
        if let Some(fpu) = from.fpu_storage.as_mut() {
            xsave(fpu);
        }

        if let Some(fpu) = to.fpu_storage.as_ref() {
            xrstor(fpu);
        }

        // Load the new thread's kernel stack pointer everywhere it's needed.
        let kstackp = to.context_switch_rsp.as_u64();
        super::gdt::TSS.rsp[0] = kstackp;
        io::wrmsr(io::IA32_SYSENTER_ESP, kstackp);

        // Preserve and restore the %fs, %gs bases.
        from.fs_base = io::get_fsbase();
        from.gs_base = io::get_inactive_gsbase();

        io::set_fsbase(to.fs_base);
        io::set_inactive_gsbase(to.gs_base);

        task_spinup(&mut from.context, to.context.as_ref());
    }
}
