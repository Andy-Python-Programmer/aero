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

use proc_macro::TokenStream;
use syn::ItemFn;

pub fn parse(_: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as ItemFn);

    let name = &input.sig.ident;
    let body = &input.block;

    let marker_name = quote::format_ident!("{}_test_marker", name);
    let result = quote::quote! {
        #[test_case]
        static #marker_name: crate::tests::Test = crate::tests::Test {
            test_fn: #name,
            path: concat!(module_path!(), "::", stringify!(#name))
        };

        fn #name() {
            #body
        }
    };

    result.into()
}
