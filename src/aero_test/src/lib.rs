use proc_macro::TokenStream;
use syn::ItemFn;

extern crate proc_macro;

#[proc_macro_attribute]
pub fn test(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as ItemFn);

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
