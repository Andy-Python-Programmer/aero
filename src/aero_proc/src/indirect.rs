use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};

pub fn parse(_: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as syn::ItemFn);
    let args = item.sig.inputs;

    let name = item.sig.ident.to_string();

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
            fn #name(#args);
        }
    }
    .into()
}
