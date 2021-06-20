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

use super::idt::PageFaultErrorCode;
use super::interrupt_error_stack;

use crate::unwind;
use x86_64::registers::control::Cr2;

macro interrupt_exception(fn $name:ident() => $message:expr) {
    super::interrupt_error_stack!(
        fn $name(stack: &mut InterruptErrorStack) {
            log::error!("EXCEPTION: {}\n\nStack: {:#x?}", $message, stack);

            if stack.stack.iret.is_user() {
                loop {}
            }

            $crate::unwind::unwind_stack_trace();

            unsafe {
                $crate::arch::interrupts::disable_interrupts();

                loop {
                    $crate::arch::interrupts::halt();
                }
            }
        }
    );
}

interrupt_exception!(fn divide_by_zero() => "Division by zero");
interrupt_exception!(fn debug() => "Debug");
interrupt_exception!(fn non_maskable() => "Non Maskable");
interrupt_exception!(fn overflow() => "Stack Overflow");
interrupt_exception!(fn bound_range() => "Out of Bounds");
interrupt_exception!(fn invalid_opcode() => "Invalid Opcode");
interrupt_exception!(fn device_not_available() => "Device not Avaliable");
interrupt_exception!(fn double_fault() => "Double Fault");
interrupt_exception!(fn invalid_tss() => "Invalid TSS");
interrupt_exception!(fn segment_not_present() => "Segment not Present");
interrupt_exception!(fn stack_segment() => "Stack Segment Fault");
interrupt_exception!(fn protection() => "Protection Fault");
interrupt_exception!(fn fpu_fault() => "FPU floating point fault");
interrupt_exception!(fn alignment_check() => "Alignment check fault");
interrupt_exception!(fn machine_check() => "Machine check fault");
interrupt_exception!(fn simd() => "SIMD floating point fault");
interrupt_exception!(fn virtualization() => "Virtualization fault");
interrupt_exception!(fn security() => "Security exception");

interrupt_error_stack!(
    fn breakpoint(stack: &mut InterruptErrorStack) {
        /*
         * We will need to prevent RIP from going out of sync with
         * instructions.
         *
         * So we will set the RIP to RIP - 1, pointing to the int3
         * instruction.
         */
        (*stack).stack.iret.rip -= 1;
    }
);

interrupt_error_stack!(
    fn page_fault(stack: &mut InterruptErrorStack) {
        let accessed_address = Cr2::read();

        log::error!(
            "EXCEPTION: Page Fault\n\nAccessed Address: {:?}\nError: {:?}\nStack: {:#x?}",
            accessed_address,
            PageFaultErrorCode::from_bits_truncate(stack.code as u64),
            stack.stack,
        );

        if stack.stack.iret.is_user() {
            loop {}
        }

        unwind::unwind_stack_trace();

        unsafe {
            super::disable_interrupts();

            loop {
                super::halt();
            }
        }
    }
);
