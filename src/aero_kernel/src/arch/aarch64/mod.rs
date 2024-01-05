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

pub mod dtb;
pub mod interrupts;
pub mod task;
pub mod time;
pub mod tls;

use crate::{drivers, logger};

use limine::*;

use crate::mem::paging::VirtAddr;

static TERMINAL: LimineTerminalRequest = LimineTerminalRequest::new(0);
static HHDM: LimineHhdmRequest = LimineHhdmRequest::new(0);
static KERNEL_FILE: LimineKernelFileRequest = LimineKernelFileRequest::new(0);
static DTB: LimineDtbRequest = LimineDtbRequest::new(0);

#[no_mangle]
extern "C" fn arch_aero_main() -> ! {
    unsafe {
        interrupts::disable_interrupts();
    }

    unsafe {
        crate::PHYSICAL_MEMORY_OFFSET = VirtAddr::new(HHDM.get_response().get().unwrap().offset);
    }

    let kernel_file_resp = KERNEL_FILE
        .get_response()
        .get()
        .expect("limine: invalid kernel file response");

    let kernel_file = kernel_file_resp
        .kernel_file
        .get()
        .expect("limine: invalid kernel file pointer");

    // Before we start the initialization process, we need to make sure
    // the unwind info is available; just in case if there is a kernel
    // panic, it will be able to unwind the stack.
    crate::unwind::UNWIND_INFO.call_once(|| {
        use crate::unwind::UnwindInfo;
        use xmas_elf::ElfFile;

        let start = kernel_file
            .base
            .as_ptr()
            .expect("limine: invalid kernel file base");

        // SAFETY: The bootloader will provide a valid pointer to the kernel file.
        let elf_slice = unsafe { core::slice::from_raw_parts(start, kernel_file.length as usize) };
        let elf = ElfFile::new(elf_slice).expect("limine: invalid kernel file");

        UnwindInfo::new(elf)
    });

    // Now that we have unwind info, we can initialize the COM ports. This
    // will be used to print panic messages/logs before the debug renderer is
    // initialized to the serial output (if available).
    drivers::uart::init();
    logger::init();

    log::debug!("lmao");
    let dtb_response = DTB.get_response().get().unwrap();
    let dtb_blob = dtb_response.dtb_ptr.as_ptr().unwrap();
    let dtb = dtb::Dtb::new(dtb_blob);

    loop {}
}
