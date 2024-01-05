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
use syn::spanned::Spanned;

pub fn parse(_: TokenStream, item: TokenStream) -> TokenStream {
    let parsed_trait = syn::parse_macro_input!(item as syn::ItemTrait);

    let vis = &parsed_trait.vis;
    let name = &parsed_trait.ident;
    let items = &parsed_trait.items;
    let generics = &parsed_trait.generics;

    // `auto` and `unsafe` traits are not allowed:
    if let Some(token) = parsed_trait.auto_token {
        emit_error!(token.span(), "`auto` traits are not downcastable")
    } else if let Some(token) = parsed_trait.unsafety {
        emit_error!(token.span(), "`unsafe` traits are not downcastable")
    }

    let super_traits = parsed_trait.supertraits.clone();

    quote::quote! {
        #vis trait #name #generics: #super_traits + crate::utils::Downcastable {
            #(#items)*
        }

        // #[downcast]: implement downcast functions:
        impl dyn #name #generics {
            /// Downcast's an `Arc`ed trait object to an `Arc`ed object if the underlying object
            /// is of type `T`.
            pub fn downcast_arc<T: #name #generics>(self: &alloc::sync::Arc<Self>) -> Option<alloc::sync::Arc<T>> {
                self.clone().as_any().downcast::<T>().ok()
            }
        }
    }
    .into()
}
