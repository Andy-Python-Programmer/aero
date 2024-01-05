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
use syn::{Lit, Meta, MetaNameValue, NestedMeta};

pub fn parse(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemStatic);

    let ty = &item.ty;
    let ident = &item.ident;
    let mutability = &item.mutability;
    let vis = &item.vis;
    let initializer = &item.expr;

    // Parse the attribute arguments
    let args = syn::parse_macro_input!(attr as syn::AttributeArgs);

    // Process each argument to find the subsection value
    let mut subsection = None;
    for arg in args {
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) = arg {
            if let Some(ident) = path.get_ident() {
                if ident == "subsection" {
                    if let Lit::Str(lit_str) = lit {
                        subsection = Some(lit_str.value());
                    }
                }
            }
        }
    }

    let link_section = match subsection {
        Some(subsection) => format!(".cpu_local_{}", subsection),
        None => ".cpu_local".to_string(),
    };

    quote::quote! {
        #[link_section = #link_section]
        #[used]
        #vis static #mutability #ident: crate::arch::cpu_local::CpuLocal<#ty> = crate::arch::cpu_local::CpuLocal::new(#initializer);
    }
    .into()
}
