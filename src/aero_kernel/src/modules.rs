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

use crate::utils::linker::LinkerSymbol;

/// Inner helper function to make sure the function provided to the [module_init] macro
/// has a valid function signature. This function returns the passed module init function as
/// a const void pointer.
#[inline]
pub const fn make_module_init(init_function: fn() -> ()) -> ModuleInit {
    ModuleInit(init_function as *const ())
}

/// Inner helper structure holding the module init function as a void pointer. This struct
/// is required as we cannot directly store a pointer in the static as it needs to implement
/// [Sync].
pub struct ModuleInit(*const ());

unsafe impl Sync for ModuleInit {}

#[macro_export]
macro_rules! module_init {
    ($init_function:expr) => {
        #[used]
        #[link_section = ".kernel_modules.init"]
        static __MODULE_INIT: $crate::modules::ModuleInit =
            $crate::modules::make_module_init($init_function);
    };
}

/// This function is responsible for initializing all of the kernel modules. Since currently
/// we cannot read the ext2 root filesystem, we link all of the kernel modules into the kernel
/// itself (this is temporary and modules will be loaded from the filesystem in the future).
pub(crate) fn init() {
    extern "C" {
        static __kernel_modules_start: LinkerSymbol;
        static __kernel_modules_end: LinkerSymbol;
    }

    /*
     * Iterate over the `kernel_modules` linker section containing pointers to module
     * initialization functions.
     */
    unsafe {
        (__kernel_modules_start.as_usize()..__kernel_modules_end.as_usize())
            .step_by(0x08)
            .for_each(|module| (*(module as *mut fn() -> ()))());
    }
}
