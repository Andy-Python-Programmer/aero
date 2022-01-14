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

use proc_macro::TokenStream;
use syn::ItemFn;

extern crate proc_macro;

#[proc_macro_attribute]
pub fn test(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as ItemFn);

    let name = &input.sig.ident;
    let body = &input.block;

    let marker_name = quote::format_ident!("{}_func", name);

    let result = quote::quote! {
        #[allow(warnings)]
        static #name: crate::Test = crate::Test {
            func: #marker_name,
            path: concat!(module_path!(), "::", stringify!(#name))
        };

        fn #marker_name() {
            #body
        }
    };

    result.into()
}
