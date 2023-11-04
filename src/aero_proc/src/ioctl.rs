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
