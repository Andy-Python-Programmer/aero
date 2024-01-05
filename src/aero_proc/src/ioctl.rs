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
use syn::{Data, DeriveInput, Path};

fn make_command_enum(ast: &DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let variants = match &ast.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("`Ioctl` derive macro can only be used on enums."),
    };

    let mut pattern_match = vec![];

    for variant in variants {
        let attrs = &variant.attrs;
        let ident = &variant.ident;

        for attr in attrs {
            if attr.path.get_ident().unwrap() != "command" {
                assert_eq!(attr.path.get_ident().unwrap(), "doc");
                continue;
            }

            let path = attr.parse_args::<Path>().unwrap();

            pattern_match.push(match &variant.fields {
                syn::Fields::Unit => quote::quote!(#path => Self::#ident),
                syn::Fields::Unnamed(fields) => {
                    assert!(fields.unnamed.len() == 1);
                    quote::quote!(#path => Self::#ident(crate::syscall::SysArg::from_usize(arg)))
                }

                _ => panic!("`Ioctl` derive macro can only be used on enums with unit variants."),
            });
        }
    }

    // implement Ioctl::from_command_arg for the enum

    quote::quote! {
        impl #name {
            pub fn from_command_arg(cmd: usize, arg: usize) -> Self {
                match cmd {
                    #(#pattern_match,)*
                    _ => unimplemented!("unknown command: {cmd:#x}")
                }
            }
        }
    }
    .into()
}

pub fn parse(item: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(item).unwrap();
    let cmd_enum = make_command_enum(&ast);

    cmd_enum
}
