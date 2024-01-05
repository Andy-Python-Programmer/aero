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
use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;

pub fn parse(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    let args = item.sig.inputs;

    if !args.is_empty() {
        abort!(args.span(), "resolver is expected to take no arguments");
    }

    let name = item.sig.ident.to_string();
    let vis = item.vis;

    let output_fn = match item.sig.output {
        syn::ReturnType::Type(_, ty) => match ty.as_ref() {
            syn::Type::BareFn(bare_fn) => bare_fn.clone(),
            ty => abort!(ty.span(), "expected output function type"),
        },
        ty => abort!(ty.span(), "expected output function type"),
    };

    let output_args = &output_fn.inputs;
    let output_ret = &output_fn.output;

    // Underscores at the beginning of the identifier make it reserved, and the more underscores
    // there are, the more reserveder it is.
    let resolve_name = Ident::new(&format!("__resolve_{name}"), Span::call_site());

    let inline = format!(
        r"
        .global {name}

        .type {name}, @gnu_indirect_function
        .set {name},{{}}
        "
    );

    let name = &item.sig.ident;
    let resolve_body = &item.block;

    quote::quote! {
        fn #resolve_name() -> usize {
            let resolved_function = {
                #resolve_body
            };

            resolved_function as usize
        }

        ::core::arch::global_asm!(#inline, sym #resolve_name);

        extern "C" {
            #vis fn #name(#output_args) #output_ret;
        }
    }
    .into()
}
