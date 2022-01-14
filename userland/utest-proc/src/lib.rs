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
