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

//! A kernel module is an object file that contains code that can extend
//! the kernel functionality at runtime. When a kernel module is no longer needed,
//! it can be unloaded. Most of the device drivers are used in the form of kernel modules.
//!
//! ## Example
//!
//! ```rust,no_run
//! fn hello_init() {}
//! fn hello_exit() {}
//!
//! aero_kernel::module_init!(hello_init);
//! aero_kernel::module_exit!(hello_exit);
//! ```

use crate::{drivers, fs};

/// Inner helper function to make sure the function provided to the [module_init] macro
/// has a valid function signature. This function returns the passed module init function as
/// a const void pointer.

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
#[repr(C)]
pub enum ModuleType {
    Block = 0,
    Other = 1,
}

#[derive(Debug)]
#[repr(C)]
pub struct Module {
    pub init: *const fn() -> (),
    pub ty: ModuleType,
}

unsafe impl Sync for Module {}

#[macro_export]
macro_rules! module_init {
    ($init_function:expr, $ty:path) => {
        use crate::modules::ModuleType;

        #[used]
        #[link_section = ".kernel_modules.init"]
        static __MODULE_INIT: $crate::modules::Module = $crate::modules::Module {
            init: $init_function as *const fn() -> (),
            ty: $ty,
        };
    };
}

/// This function is responsible for initializing all of the kernel modules. Since currently
/// we cannot read the ext2 root filesystem, we link all of the kernel modules into the kernel
/// itself (this is temporary and modules will be loaded from the filesystem in the future).
pub(crate) fn init() {
    extern "C" {
        static mut __kernel_modules_start: u8;
        static mut __kernel_modules_end: u8;
    }

    unsafe {
        let size = &__kernel_modules_end as *const u8 as usize
            - &__kernel_modules_start as *const u8 as usize;

        let modules = core::slice::from_raw_parts_mut(
            &mut __kernel_modules_start as *mut u8 as *mut Module,
            size / core::mem::size_of::<Module>(),
        );

        modules.sort_by(|e, a| e.ty.cmp(&a.ty));

        let mut launched_fs = false;

        for module in modules {
            log::debug!("{module:?} {launched_fs}");

            if module.ty != ModuleType::Block && !launched_fs {
                let mut address_space = crate::mem::AddressSpace::this();
                let mut offset_table = address_space.offset_page_table();

                #[cfg(target_arch = "x86_64")]
                drivers::pci::init(&mut offset_table);
                log::info!("loaded PCI driver");

                fs::block::launch().unwrap();
                launched_fs = true;
            }

            let init = core::mem::transmute::<*const fn() -> (), fn() -> ()>(module.init);
            init();
        }
    }
}
