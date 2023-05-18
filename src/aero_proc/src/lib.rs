// Copyright (C) 2021-2023 The Aero Project Developers.
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

#![feature(proc_macro_diagnostic, proc_macro_span)]

#[macro_use]
extern crate proc_macro_error;

mod downcastable;
mod syscall;
mod test;

use proc_macro::TokenStream;

/// Support for kernel unit-testing framework.
///
/// ## Example
/// ```rust,no_run
/// #[test]
/// fn some_test() {
///     assert_eq!(2 + 2, 4);
/// }
/// ```
#[proc_macro_attribute]
#[proc_macro_error]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    test::parse(attr, item)
}

/// Validates input buffers, structures, path and strings auto-magically.
///
/// Functions that use this macro are not allowed to be `async`, `unsafe`, or `const` and must
/// have a valid return-type of `Result<usize, AeroSyscallError>`. In addition, the function cannot
/// have generic parameters.
#[proc_macro_attribute]
#[proc_macro_error]
pub fn syscall(attr: TokenStream, item: TokenStream) -> TokenStream {
    syscall::parse(attr, item)
}

#[proc_macro_attribute]
#[proc_macro_error]
pub fn downcastable(attr: TokenStream, item: TokenStream) -> TokenStream {
    downcastable::parse(attr, item)
}
